// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Project readiness** (one login, slice D): whether a project's forge would let *every other
//! person with access to the repo* sign in to purlis and work, and, for each thing that would
//! not, who can fix it and how.
//!
//! One person sets a project up with their own login. The forge-side settings that block
//! somebody else (an organization that has not approved purlis's OAuth app, a server that does
//! not know purlis, a read-only role, an unprotected default branch) are usually not that
//! person's to change. So each check answers one of three, and only one means "go":
//!
//! - [`Verdict::Ready`]: purlis saw the setting and it is fine;
//! - [`Verdict::Blocked`]: purlis saw the setting and it blocks someone, and [`Who`] can fix it;
//! - [`Verdict::Unknown`]: purlis could not see it (a refusal, a 404 that hides the answer, a
//!   field the forge did not send). **Unknown is never ready**: a [`Report`] is
//!   [`Report::ready_for_all`] only when every check that applies is `Ready`.
//!
//! Every blocker carries [`Fix`]: the clicks, with exact URLs, and, when the person asking is
//! not the one who can fix it, a short request text addressed to them.
//!
//! # Where the requests are built
//!
//! The forge-specific reads are [`Probe`], implemented once per forge in `github/readiness.rs`
//! and `gitlab/readiness.rs`, which keeps "nothing outside those two builds a request" true.
//! [`evaluate`] is pure: facts in, a [`Report`] out, so every check is tested without a forge.
//! The default branch's protection is read through the seam's own
//! [`Requests::ruleset_protects`](super::backend::Requests::ruleset_protects), the merge hold's
//! reading, so the two never disagree.
//!
//! Nothing here is stored. A report is derived from the forge each time it is asked for (on
//! demand, on project open, and by `purlis doctor`), so there is no tier to name.

use std::sync::Arc;
use std::time::Duration;

use super::accounts::Method;
use super::backend::{Caller, Fixed, ForgeBackend, Kept, PROBE_KEPT_FOR, Transports};
use super::transport::Transport;
use super::{Forge, ForgeError, Kind, registration};

/// How the person is connected to the forge, which decides the checks that apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connection {
    /// Signed in through **purlis's own** OAuth app, in the window: subject to an
    /// organization's OAuth app approval and to the host knowing the app. It is the only
    /// connection whose reads prove anything about that approval.
    SignIn,
    /// Read over the `gh` or `glab` login (what `purlis doctor` uses, and the window where the
    /// person has not signed in to purlis on the host), or an imported one. That is the CLI's
    /// OAuth app, not purlis's: what it reads says nothing of whether the organization approved
    /// purlis, so that check is unknown.
    Cli,
    /// A token pasted in for the host: not subject to OAuth app approval; its workflow
    /// permissions are checked instead.
    Token,
}

impl Connection {
    /// The connection a credential is, from how its account was signed in: only purlis's own
    /// OAuth flows ([`Method::Device`], [`Method::Pkce`]) are [`Connection::SignIn`]; an imported
    /// CLI login is the CLI's ([`Connection::Cli`], which is also no account at all); a pasted
    /// token is [`Connection::Token`]. What the environment says (`CI` being set) decides
    /// nothing: only a token configured for the host is one.
    pub fn of_method(method: Option<Method>) -> Connection {
        match method {
            Some(Method::Device | Method::Pkce) => Connection::SignIn,
            Some(Method::Pat) => Connection::Token,
            Some(Method::Import) | None => Connection::Cli,
        }
    }

    /// Whether this is purlis's own sign-in, the only connection that proves an approval.
    pub fn is_sign_in(self) -> bool {
        self == Connection::SignIn
    }
}

/// What a probe could or could not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fact<T> {
    Known(T),
    /// Not seen, and why, on one line.
    Unknown(String),
}

impl<T> Fact<T> {
    pub fn unknown(why: impl Into<String>) -> Fact<T> {
        Fact::Unknown(why.into())
    }
}

/// The account's own part in the repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// Reads, and may not push.
    Read,
    /// May push (GitHub write; GitLab Developer).
    Write,
    /// Administers the repo (GitHub admin; GitLab Maintainer or Owner).
    Admin,
}

/// Whether the organization has approved purlis's OAuth app (GitHub's "OAuth app access
/// restrictions").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrgAccess {
    /// A private repo of the organization was read with the sign-in: it is approved, or the
    /// organization has no restriction.
    Approved,
    /// GitHub refused the sign-in by name: the organization restricts OAuth apps and has not
    /// approved this one.
    Restricted,
    /// The repo is public, which an unapproved app reads too, so nothing here proves approval.
    Unproven,
    /// Not an organization's repo, or not GitHub: nothing to approve.
    NotApplicable,
}

/// What reading the repo itself gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoFacts {
    /// The organization, group or user whose namespace the repo is in.
    pub owner: String,
    /// Whether that namespace is an organization or group, rather than a person.
    pub owner_is_group: bool,
    pub default_branch: String,
    pub role: Role,
    /// Whether the account owns the organization or group (GitHub org owner; GitLab Owner).
    pub org_owner: bool,
    /// Whether the account administers the instance (GitHub `site_admin`; GitLab `is_admin`).
    pub instance_admin: bool,
    pub org_access: OrgAccess,
}

/// What reading the repo came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoRead {
    Read(RepoFacts),
    /// The forge refused with a 403 that names the organization's OAuth app restriction. The
    /// repo itself was not read, so only that is known.
    Restricted(String),
    /// A 403 that says the organization enforces SAML single sign-on (`X-GitHub-SSO`): the
    /// credential is not authorized for it.
    Sso(String),
    /// A 403 that names no cause. It is the OAuth app restriction or an IP allow list, and
    /// nothing purlis can read tells which (ADR D10), so it is guided and never judged.
    Refused(String),
    Unknown(String),
}

/// The forge-specific reads. Every method answers, never fails: what could not be read is
/// [`Fact::Unknown`] with its reason.
pub trait Probe {
    /// The repo at `path` as the account sees it.
    fn repo(&self, caller: &Caller, path: &str) -> RepoRead;
    /// Whether the instance lets users register their own OAuth applications (GitLab's
    /// instance setting; only an instance admin can read it).
    fn user_apps(&self, caller: &Caller) -> Fact<bool>;
    /// Whether a token inside CI may push to the repo at `path`: GitHub's default workflow
    /// permission, GitLab's job-token push permission.
    fn ci_token(&self, caller: &Caller, path: &str) -> Fact<bool>;
}

impl Forge {
    /// This forge's readiness probe over `transport`, for every caller.
    pub fn probe_over(&self, transport: Arc<dyn Transport>) -> Box<dyn Probe> {
        self.probe_for(Arc::new(Fixed(transport)))
    }

    /// This forge's readiness probe over the transports this process gives it: the person's
    /// sign-in in the window, else the CLI ([`Forge::backend`]'s own choice).
    pub fn probe(&self) -> Box<dyn Probe> {
        self.probe_for(self.process_transports())
    }

    fn probe_for(&self, transports: Arc<dyn Transports>) -> Box<dyn Probe> {
        let asker = super::backend::Asker {
            forge: self.clone(),
            transports: Arc::new(Kept::over(transports, PROBE_KEPT_FOR)),
        };
        match self.kind {
            Kind::GitHub => Box::new(super::github::GitHub(asker)),
            Kind::GitLab => Box::new(super::gitlab::GitLab(asker)),
        }
    }
}

/// Who can fix a blocker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who {
    /// The person asking can, from their own account.
    You,
    /// The organization or group's owner, or the repo's admin.
    Owner,
    /// The instance's administrator (a self-hosted server's).
    Admin,
}

impl Who {
    pub fn word(self) -> &'static str {
        match self {
            Who::You => "you",
            Who::Owner => "owner",
            Who::Admin => "admin",
        }
    }
}

/// One check's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Ready,
    Blocked(Who),
    /// Could not be read. Not ready.
    Unknown,
}

/// The things checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckId {
    /// The account's role in the repo.
    Role,
    /// The organization's approval of purlis's OAuth app (GitHub).
    OauthAppAccess,
    /// purlis's OAuth app exists on this self-hosted server.
    ServerRegistration,
    /// The instance lets users register OAuth applications (GitLab, self-hosted).
    OauthApplications,
    /// The default branch is protected: the merge hold's backstop.
    BranchProtection,
    /// A token used in CI may push.
    CiToken,
}

impl CheckId {
    /// A stable word for `--json` and the window.
    pub fn word(self) -> &'static str {
        match self {
            CheckId::Role => "role",
            CheckId::OauthAppAccess => "oauth-app-access",
            CheckId::ServerRegistration => "server-registration",
            CheckId::OauthApplications => "oauth-applications",
            CheckId::BranchProtection => "branch-protection",
            CheckId::CiToken => "ci-token",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            CheckId::Role => "Your role in the repo",
            CheckId::OauthAppAccess => "Organization approval of purlis",
            CheckId::ServerRegistration => "purlis registered on this server",
            CheckId::OauthApplications => "Server allows OAuth applications",
            CheckId::BranchProtection => "Default branch protected",
            CheckId::CiToken => "CI token may push",
        }
    }
}

/// How to fix one blocker (or look at one unknown).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fix {
    /// Where the setting is.
    pub link: String,
    /// The clicks, in order, each naming what to press.
    pub steps: Vec<String>,
    /// A short message to send to whoever can fix it. `None` when the person can fix it
    /// themselves.
    pub request: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub id: CheckId,
    pub verdict: Verdict,
    /// What was found, on one line.
    pub detail: String,
    /// Present for a blocker, and for an unknown that has a way to find out.
    pub fix: Option<Fix>,
}

/// Every check that applies to one project on one host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub kind: Kind,
    pub host: String,
    pub path: String,
    pub checks: Vec<Check>,
}

impl Report {
    /// **Ready for every user**: at least one check, and every one of them `Ready`. A check that
    /// could not be read keeps this false.
    pub fn ready_for_all(&self) -> bool {
        !self.checks.is_empty() && self.checks.iter().all(|c| c.verdict == Verdict::Ready)
    }

    pub fn blockers(&self) -> impl Iterator<Item = &Check> {
        self.checks
            .iter()
            .filter(|c| matches!(c.verdict, Verdict::Blocked(_)))
    }

    pub fn unknowns(&self) -> impl Iterator<Item = &Check> {
        self.checks.iter().filter(|c| c.verdict == Verdict::Unknown)
    }
}

/// One host's state across the projects checked on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostState {
    /// Every project checked is ready for all users.
    Ready,
    /// Something blocks or could not be read: how many projects are not ready.
    NotReady { projects: usize },
    /// Nothing has been checked on this host.
    NotChecked,
}

/// The state of a host from the reports of its projects. Empty is [`HostState::NotChecked`],
/// never `Ready`.
pub fn host_state(reports: &[Report]) -> HostState {
    if reports.is_empty() {
        return HostState::NotChecked;
    }
    match reports.iter().filter(|r| !r.ready_for_all()).count() {
        0 => HostState::Ready,
        projects => HostState::NotReady { projects },
    }
}

/// Everything [`evaluate`] needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    pub repo: RepoRead,
    /// Whether the default branch is protected, as the seam read it.
    pub protected: Fact<bool>,
    /// GitLab only, and read only for a self-hosted server without a registration.
    pub user_apps: Fact<bool>,
    /// Read only for [`Connection::Token`].
    pub ci_token: Fact<bool>,
    /// Whether purlis has an OAuth application on the host: one it ships, or one the person
    /// pinned there ([`super::registered`]). Unknown where the pins could not be read. A host
    /// purlis ships a registration on is registered whatever this says.
    pub registered: Fact<bool>,
}

/// Whether purlis has an OAuth application on `on`'s host, read from `config_root`'s pins. No
/// config home holds no pin; a pin file that cannot be read is unknown, never "none".
pub(super) fn registered_in(config_root: Option<&std::path::Path>, on: &Forge) -> Fact<bool> {
    if registration::on(on.kind, &on.host).is_some() {
        return Fact::Known(true);
    }
    let Some(root) = config_root else {
        return Fact::Known(false);
    };
    match super::registered::pinned(root, on.kind, &on.host) {
        Ok(pin) => Fact::Known(pin.is_some()),
        Err(why) => Fact::Unknown(one_line(&why)),
    }
}

/// Check a project: read what the forge says as `caller` over `transport`, and judge it.
///
/// It asks the network, several requests: call it off the UI thread. A request that fails is
/// that check's `Unknown`, never an error here.
pub fn check(
    on: &Forge,
    path: &str,
    connection: Connection,
    caller: &Caller,
    transport: Arc<dyn Transport>,
) -> Report {
    check_with(
        on,
        path,
        connection,
        caller,
        on.probe_over(transport.clone()).as_ref(),
        on.backend_over(transport).as_ref(),
    )
}

/// [`check`] as the window: over the person's purlis sign-in where this process has one, and
/// otherwise over the CLI, with the [`Connection`] the call really takes
/// ([`super::signin::connection`]).
pub fn check_in_window(on: &Forge, path: &str, caller: &Caller) -> Report {
    check_with(
        on,
        path,
        super::signin::connection(on, caller),
        caller,
        on.probe().as_ref(),
        on.backend().as_ref(),
    )
}

/// [`check`] over a probe and a backend the caller chose.
pub fn check_with(
    on: &Forge,
    path: &str,
    connection: Connection,
    caller: &Caller,
    probe: &dyn Probe,
    backend: &dyn ForgeBackend,
) -> Report {
    let repo = probe.repo(caller, path);
    let registered = registered_in(crate::machine::config_root().as_deref(), on);
    let needs_app_policy = connection != Connection::Token
        && on.kind == Kind::GitLab
        && registered == Fact::Known(false);
    let user_apps = if needs_app_policy {
        probe.user_apps(caller)
    } else {
        Fact::unknown("not read")
    };
    let ci_token = if connection == Connection::Token {
        probe.ci_token(caller, path)
    } else {
        Fact::unknown("not read")
    };
    let protected = match &repo {
        RepoRead::Read(facts) => protection(backend, path, &facts.default_branch, caller),
        RepoRead::Restricted(_) | RepoRead::Sso(_) | RepoRead::Refused(_) => {
            Fact::unknown("the repo could not be read")
        }
        RepoRead::Unknown(why) => Fact::unknown(format!("the repo could not be read: {why}")),
    };
    evaluate(
        on,
        path,
        connection,
        &Facts {
            repo,
            protected,
            user_apps,
            ci_token,
            registered,
        },
    )
}

/// Check the project whose remote is `url`, in the plane at `root`: which forge it is on and its
/// path are read from the URL. `Err` is why there is nothing to check (a path on this machine, a
/// host no forge purlis knows names, no repository in the URL), on one line.
pub fn check_url(
    url: &str,
    root: &std::path::Path,
    connection: Connection,
    caller: &Caller,
    transport: Arc<dyn Transport>,
) -> Result<Report, String> {
    let (on, path) = target(url, root)?;
    Ok(check(&on, &path, connection, caller, transport))
}

/// [`check_url`] as the window, over the person's sign-in ([`check_in_window`]).
pub fn check_url_in_window(
    url: &str,
    root: &std::path::Path,
    caller: &Caller,
) -> Result<Report, String> {
    let (on, path) = target(url, root)?;
    Ok(check_in_window(&on, &path, caller))
}

/// The forge and repo path a remote URL names, or why it names none.
fn target(url: &str, root: &std::path::Path) -> Result<(Forge, String), String> {
    if super::host_of(url).is_empty() {
        return Err("origin is on this machine, so nobody else signs in to it".to_owned());
    }
    let Some(on) = super::resolve_host(url, root) else {
        return Err(format!(
            "origin is on {}, a host no forge purlis knows names",
            one_line(&super::host_of(url))
        ));
    };
    let Some(path) = super::namespace_of(url) else {
        return Err("origin names no repository purlis can read".to_owned());
    };
    Ok((on, path))
}

fn protection(backend: &dyn ForgeBackend, path: &str, branch: &str, caller: &Caller) -> Fact<bool> {
    match backend.ruleset_protects(caller, path, branch) {
        Ok(protected) => Fact::Known(protected),
        Err(e) => Fact::Unknown(one_line(e.said())),
    }
}

/// A forge's own words on one line, cut.
pub(super) fn one_line(said: &str) -> String {
    let line: String = said
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(160)
        .collect();
    if line.is_empty() {
        "the forge gave no reason".to_owned()
    } else {
        line
    }
}

/// Judge `facts`: pure, so each check is tested alone. The role is always checked, so a project
/// whose repo could not be read is never ready.
pub fn evaluate(on: &Forge, path: &str, connection: Connection, facts: &Facts) -> Report {
    let words = Words::of(on, path);
    let mut checks = Vec::new();
    let person = match &facts.repo {
        RepoRead::Read(f) => Some(f),
        _ => None,
    };

    // The role.
    checks.push(match &facts.repo {
        RepoRead::Read(f) if f.role >= Role::Write => Check {
            id: CheckId::Role,
            verdict: Verdict::Ready,
            detail: format!("you can push to {path}"),
            fix: None,
        },
        RepoRead::Read(f) => {
            let fix = words.role_fix(f);
            Check {
                id: CheckId::Role,
                verdict: Verdict::Blocked(fix.0),
                detail: format!("you can read {path} but not push to it"),
                fix: Some(fix.1),
            }
        }
        RepoRead::Restricted(_) => Check {
            id: CheckId::Role,
            verdict: Verdict::Unknown,
            detail: "your role could not be read: the organization refused purlis".to_owned(),
            fix: None,
        },
        RepoRead::Sso(_) => Check {
            id: CheckId::Role,
            verdict: Verdict::Unknown,
            detail: "your role could not be read: the organization requires SAML single sign-on"
                .to_owned(),
            fix: None,
        },
        RepoRead::Refused(_) => Check {
            id: CheckId::Role,
            verdict: Verdict::Unknown,
            detail: "your role could not be read: GitHub refused the read (403) and named no cause"
                .to_owned(),
            fix: None,
        },
        RepoRead::Unknown(why) => Check {
            id: CheckId::Role,
            verdict: Verdict::Unknown,
            detail: format!("your role could not be read: {why}"),
            fix: None,
        },
    });

    // The organization's approval (GitHub). Only a read over purlis's own sign-in says anything
    // about it: a read over the CLI's login is the CLI's OAuth app, so it is unknown.
    if on.kind == Kind::GitHub
        && connection != Connection::Token
        && let Some(check) = approval(&words, connection, &facts.repo, path)
    {
        checks.push(check);
    }

    // purlis registered on a self-hosted server (anyone who signs in to purlis there): shipped
    // or pinned. Pins that could not be read leave it unknown, never blocked or ready.
    let registered = if registration::on(on.kind, &on.host).is_some() {
        Fact::Known(true)
    } else {
        facts.registered.clone()
    };
    if connection != Connection::Token
        && let Fact::Unknown(why) = &registered
    {
        checks.push(Check {
            id: CheckId::ServerRegistration,
            verdict: Verdict::Unknown,
            detail: format!(
                "whether purlis has an OAuth app on {} was not read: {why}",
                on.host
            ),
            fix: Some(words.registration_fix(person).1),
        });
    }
    if connection != Connection::Token && registered == Fact::Known(false) {
        let (who, fix) = words.registration_fix(person);
        checks.push(Check {
            id: CheckId::ServerRegistration,
            verdict: Verdict::Blocked(who),
            detail: format!(
                "purlis has no OAuth app on {}, so nobody can sign in there through purlis; \
                 a token works meanwhile",
                on.host
            ),
            fix: Some(fix),
        });
        if on.kind == Kind::GitLab {
            checks.push(match &facts.user_apps {
                Fact::Known(true) => Check {
                    id: CheckId::OauthApplications,
                    verdict: Verdict::Ready,
                    detail: format!("{} lets users register OAuth applications", on.host),
                    fix: None,
                },
                Fact::Known(false) => {
                    let (who, fix) = words.apps_fix(person);
                    Check {
                        id: CheckId::OauthApplications,
                        verdict: Verdict::Blocked(who),
                        detail: format!(
                            "{} does not let users register OAuth applications",
                            on.host
                        ),
                        fix: Some(fix),
                    }
                }
                Fact::Unknown(why) => Check {
                    id: CheckId::OauthApplications,
                    verdict: Verdict::Unknown,
                    detail: format!("not read: {why}"),
                    fix: Some(words.apps_look()),
                },
            });
        }
    }

    // The default branch's protection.
    checks.push(match &facts.protected {
        Fact::Known(true) => Check {
            id: CheckId::BranchProtection,
            verdict: Verdict::Ready,
            detail: match person {
                Some(f) => format!("{} is protected", f.default_branch),
                None => "the default branch is protected".to_owned(),
            },
            fix: None,
        },
        Fact::Known(false) => {
            let (who, fix) = words.protection_fix(person);
            Check {
                id: CheckId::BranchProtection,
                verdict: Verdict::Blocked(who),
                detail: "nothing protects the default branch, so a push or merge there is not held"
                    .to_owned(),
                fix: Some(fix),
            }
        }
        Fact::Unknown(why) => Check {
            id: CheckId::BranchProtection,
            verdict: Verdict::Unknown,
            detail: format!("whether the default branch is protected was not read: {why}"),
            fix: Some(words.protection_look(person)),
        },
    });

    // A token inside CI.
    if connection == Connection::Token {
        checks.push(match &facts.ci_token {
            Fact::Known(true) => Check {
                id: CheckId::CiToken,
                verdict: Verdict::Ready,
                detail: "a CI token may push to the repo".to_owned(),
                fix: None,
            },
            Fact::Known(false) => {
                let (who, fix) = words.ci_fix(person);
                Check {
                    id: CheckId::CiToken,
                    verdict: Verdict::Blocked(who),
                    detail: "a CI token may only read the repo".to_owned(),
                    fix: Some(fix),
                }
            }
            Fact::Unknown(why) => Check {
                id: CheckId::CiToken,
                verdict: Verdict::Unknown,
                detail: format!("what a CI token may do was not read: {why}"),
                fix: Some(words.ci_look()),
            },
        });
    }

    Report {
        kind: on.kind,
        host: on.host.clone(),
        path: path.to_owned(),
        checks,
    }
}

/// The organization approval's check, or none where there is nothing to approve (a personal
/// repo). **Only purlis's own sign-in can show "ready"**; every other connection, and every
/// refusal that does not name the restriction, is unknown (and guided).
fn approval(
    words: &Words<'_>,
    connection: Connection,
    repo: &RepoRead,
    path: &str,
) -> Option<Check> {
    let id = CheckId::OauthAppAccess;
    let unknown = |detail: String, fix: Fix| {
        Some(Check {
            id,
            verdict: Verdict::Unknown,
            detail,
            fix: Some(fix),
        })
    };
    if !connection.is_sign_in() {
        // Whatever the CLI's login read (or was refused) is not purlis's approval.
        let by = "this was read over the CLI's login, which is not purlis's own sign-in, so \
                  it shows nothing about whether the organization approved purlis";
        return match repo {
            RepoRead::Read(f) if f.org_access == OrgAccess::NotApplicable => None,
            RepoRead::Read(f) => unknown(format!("{}: {by}", f.owner), words.cli_look(Some(f))),
            _ => unknown(
                format!("{}: {by}", words.owner_of(None)),
                words.cli_look(None),
            ),
        };
    }
    match repo {
        RepoRead::Restricted(said) => {
            let (who, fix) = words.approval_fix(None);
            Some(Check {
                id,
                verdict: Verdict::Blocked(who),
                detail: format!(
                    "the organization restricts OAuth apps and has not approved purlis: {said}"
                ),
                fix: Some(fix),
            })
        }
        RepoRead::Sso(_) => unknown(
            format!(
                "{} requires SAML single sign-on, and purlis's sign-in is not authorized for it",
                words.owner_of(None)
            ),
            words.sso_fix(),
        ),
        RepoRead::Refused(_) => unknown(
            format!(
                "GitHub refused the read of {path} with a 403 that names no cause: that is \
                 either {}'s OAuth app restriction or an IP allow list, and purlis cannot tell \
                 which, so it cannot say purlis is approved",
                words.owner_of(None)
            ),
            words.refused_look(),
        ),
        RepoRead::Read(f) => match f.org_access {
            OrgAccess::NotApplicable => None,
            OrgAccess::Approved => Some(Check {
                id,
                verdict: Verdict::Ready,
                detail: format!("{} lets purlis read its private repos", f.owner),
                fix: None,
            }),
            OrgAccess::Restricted => {
                let (who, fix) = words.approval_fix(Some(f));
                Some(Check {
                    id,
                    verdict: Verdict::Blocked(who),
                    detail: format!("{} has not approved purlis", f.owner),
                    fix: Some(fix),
                })
            }
            OrgAccess::Unproven => unknown(
                format!(
                    "{path} is public, which an unapproved app can read too, so nothing shows \
                     whether {} approved purlis",
                    f.owner
                ),
                words.approval_look(f),
            ),
        },
        RepoRead::Unknown(why) => {
            unknown(format!("not read: {why}"), words.approval_look_unowned())
        }
    }
}

/// How long a check waits on the forge: the forge seam's own list budget.
pub const CHECK_TIMEOUT: Duration = super::LIST_TIMEOUT;

/// The wording and URLs of one project's fixes.
struct Words<'a> {
    kind: Kind,
    host: &'a str,
    path: &'a str,
}

impl<'a> Words<'a> {
    fn of(on: &'a Forge, path: &'a str) -> Words<'a> {
        Words {
            kind: on.kind,
            host: &on.host,
            path,
        }
    }

    fn base(&self) -> String {
        format!("https://{}", self.host)
    }

    fn repo_url(&self) -> String {
        match self.kind {
            Kind::GitHub => format!("{}/{}", self.base(), self.path),
            Kind::GitLab => format!("{}/{}", self.base(), self.path),
        }
    }

    fn owner_of(&self, f: Option<&RepoFacts>) -> String {
        f.map(|f| f.owner.clone())
            .unwrap_or_else(|| self.path.split('/').next().unwrap_or(self.path).to_owned())
    }

    /// Who can fix something an owner of the organization or repo can, given `person`.
    fn owner_fixer(&self, person: Option<&RepoFacts>) -> Who {
        match person {
            Some(f) if f.org_owner || f.role == Role::Admin || f.instance_admin => Who::You,
            _ => Who::Owner,
        }
    }

    fn admin_fixer(&self, person: Option<&RepoFacts>) -> Who {
        match person {
            Some(f) if f.instance_admin => Who::You,
            _ => Who::Admin,
        }
    }

    /// The request text for whoever must act, or none when it is the person.
    fn request(&self, who: Who, ask: &str) -> Option<String> {
        let to = match who {
            Who::You => return None,
            Who::Owner => "the owner",
            Who::Admin => "the administrator",
        };
        Some(format!(
            "Hello, I am setting up purlis for {path} on {host}, so that everyone who works on it \
             can sign in once and use it. {ask} Could {to} do that? Thank you.",
            path = self.path,
            host = self.host
        ))
    }

    fn role_fix(&self, f: &RepoFacts) -> (Who, Fix) {
        let who = self.owner_fixer(Some(f));
        let (link, role, click) = match self.kind {
            Kind::GitHub => (
                format!("{}/settings/access", self.repo_url()),
                "Write",
                "Add people (or change the role in the Manage access list)",
            ),
            Kind::GitLab => (
                format!("{}/-/project_members", self.repo_url()),
                "Developer",
                "Invite members (or change the role in the list)",
            ),
        };
        let steps = vec![
            format!("Open {link}."),
            format!("Press {click}."),
            format!("Give the account the {role} role, then save."),
        ];
        let request = self.request(
            who,
            &format!("My account can read {} but not push, so purlis cannot do its work. Please give it the {role} role: {link}.", self.path),
        );
        (
            who,
            Fix {
                link,
                steps,
                request,
            },
        )
    }

    fn approval_fix(&self, f: Option<&RepoFacts>) -> (Who, Fix) {
        let org = self.owner_of(f);
        let who = match f {
            Some(f) if f.org_owner => Who::You,
            _ => Who::Owner,
        };
        let link = format!(
            "{}/organizations/{org}/settings/oauth_application_policy",
            self.base()
        );
        let app = registration::on(self.kind, self.host)
            .map(|r| {
                format!(
                    "{}/settings/connections/applications/{}",
                    self.base(),
                    r.client_id
                )
            })
            .unwrap_or_else(|| format!("{}/settings/applications", self.base()));
        let steps = if who == Who::You {
            vec![
                format!("Open {link}."),
                "Find purlis in the list of applications.".to_owned(),
                "Press Review, then Grant access (or Approve).".to_owned(),
            ]
        } else {
            vec![
                format!("Open {app} (you, signed in as yourself)."),
                format!("Next to {org}, press Request (or Grant if it shows one)."),
                format!("The owner of {org} then approves it at {link}."),
            ]
        };
        let request = self.request(
            who,
            &format!(
                "{org} restricts OAuth apps, so purlis is refused for everyone in it until it is approved. \
                 Please open {link}, find purlis (requested from {app}) and press Grant access."
            ),
        );
        (
            who,
            Fix {
                link,
                steps,
                request,
            },
        )
    }

    /// An unknown approval on a public repo: how to see it.
    fn approval_look(&self, f: &RepoFacts) -> Fix {
        let link = format!(
            "{}/organizations/{}/settings/oauth_application_policy",
            self.base(),
            f.owner
        );
        Fix {
            link: link.clone(),
            steps: vec![
                format!("Open {link} (an owner of {} can see it).", f.owner),
                "Look for purlis in the list: Approved means every member can sign in.".to_owned(),
                "If the page says third-party access is not restricted, nothing needs approving."
                    .to_owned(),
            ],
            request: self.request(
                Who::Owner,
                &format!(
                    "Please confirm that {} has approved purlis's OAuth app at {link}.",
                    f.owner
                ),
            ),
        }
    }

    /// An approval read over the CLI's login: it proves nothing, so how to see it.
    fn cli_look(&self, f: Option<&RepoFacts>) -> Fix {
        let mut fix = match f {
            Some(f) => self.approval_look(f),
            None => self.approval_look_unowned(),
        };
        fix.steps.insert(
            0,
            "Sign in to purlis on this host in Settings, then read the project's readiness again: only purlis's own \
             sign-in can show whether the organization approved it."
                .to_owned(),
        );
        fix
    }

    /// A 403 for SAML: the credential is not authorized for the organization's single sign-on.
    fn sso_fix(&self) -> Fix {
        let org = self.owner_of(None);
        let link = format!("{}/settings/connections/applications", self.base());
        Fix {
            link: link.clone(),
            steps: vec![
                format!("Open {link}."),
                "Open purlis in the list of authorized OAuth apps.".to_owned(),
                format!(
                    "Next to {org}, press Grant (SAML single sign-on), then sign in with your identity provider."
                ),
                "Back in purlis, read the project's readiness again.".to_owned(),
            ],
            request: None,
        }
    }

    /// A 403 that names no cause: guided, never judged. Both causes, and the cases nothing can
    /// detect (ADR D10).
    fn refused_look(&self) -> Fix {
        let org = self.owner_of(None);
        let link = format!(
            "{}/organizations/{org}/settings/oauth_application_policy",
            self.base()
        );
        let allow = format!("{}/organizations/{org}/settings/security", self.base());
        Fix {
            link: link.clone(),
            steps: vec![
                "purlis cannot detect which of these refused it, so look at each.".to_owned(),
                format!("OAuth app restriction: open {link}. If purlis is listed as Approved, it is not this; if not, press Grant access (an owner) or Request (a member)."),
                format!("IP allow list: open {allow} and check the address you work from is on the list."),
                format!("If {org} uses Enterprise Managed Users, or you are an outside collaborator, you may not be able to request approval: ask an owner of {org} to approve purlis for you."),
            ],
            request: self.request(
                Who::Owner,
                &format!(
                    "GitHub refuses purlis on {org}'s repos with a 403 that names no cause. Please check the OAuth app restriction at {link} (approve purlis) and the IP allow list at {allow}."
                ),
            ),
        }
    }

    fn approval_look_unowned(&self) -> Fix {
        let org = self.owner_of(None);
        let link = format!("{}/settings/connections/applications", self.base());
        Fix {
            link: link.clone(),
            steps: vec![
                format!("Open {link}."),
                format!(
                    "Open purlis and look next to {org}: a green tick is approved, a red cross is not."
                ),
            ],
            request: None,
        }
    }

    fn registration_fix(&self, person: Option<&RepoFacts>) -> (Who, Fix) {
        // The instance's administrator, or the owner of the organization or group the repo is
        // in: either may register an application there, so an owner can fix it too.
        let who = match person {
            Some(f) if f.instance_admin || (f.owner_is_group && f.org_owner) => Who::You,
            _ => Who::Admin,
        };
        let owner = self.owner_of(person);
        let (link, steps) = match self.kind {
            Kind::GitHub => {
                let link = format!("{}/settings/applications/new", self.base());
                (
                    link.clone(),
                    vec![
                        format!("Open {link} (or the organization's Developer settings)."),
                        "Name it purlis and set any homepage and callback URL.".to_owned(),
                        "Tick Enable Device Flow, then press Register application.".to_owned(),
                        "Give purlis the client id the page shows.".to_owned(),
                    ],
                )
            }
            Kind::GitLab => {
                let link = format!("{}/admin/applications/new", self.base());
                (
                    link.clone(),
                    vec![
                        format!("Open {link}."),
                        "Name it purlis; leave Confidential unticked.".to_owned(),
                        format!(
                            "Set the redirect URIs to {} (one per line).",
                            registration::REDIRECT_PORTS
                                .map(registration::redirect_uri)
                                .join(", ")
                        ),
                        "Tick the api scope, then press Save application.".to_owned(),
                        "Give purlis the application id the page shows.".to_owned(),
                    ],
                )
            }
        };
        let group_link = person
            .filter(|f| f.owner_is_group)
            .map(|_| match self.kind {
                Kind::GitHub => format!(
                    "{}/organizations/{owner}/settings/applications/new",
                    self.base()
                ),
                Kind::GitLab => format!("{}/groups/{owner}/-/settings/applications", self.base()),
            });
        let mut steps = steps;
        if let Some(group) = &group_link {
            steps.push(format!(
                "Or, as an owner of {owner}, register it for {owner} alone at {group}."
            ));
        }
        let request = self.request(
            who,
            &format!(
                "purlis needs an OAuth application on {}. Please register one at {link}{} and send me its client id.",
                self.host,
                group_link
                    .map(|g| format!(" (or, as an owner of {owner}, at {g})"))
                    .unwrap_or_default()
            ),
        );
        (
            who,
            Fix {
                link,
                steps,
                request,
            },
        )
    }

    fn apps_fix(&self, person: Option<&RepoFacts>) -> (Who, Fix) {
        let who = self.admin_fixer(person);
        let link = format!("{}/admin/application_settings/general", self.base());
        let steps = vec![
            format!("Open {link}."),
            "Expand Account and limit.".to_owned(),
            "Tick Allow users to register any application to use GitLab as an OAuth provider."
                .to_owned(),
            "Press Save changes.".to_owned(),
        ];
        let request = self.request(
            who,
            &format!(
                "{} does not let users register OAuth applications. Please allow it at {link} \
                 (Account and limit), or register an instance application for purlis.",
                self.host
            ),
        );
        (
            who,
            Fix {
                link,
                steps,
                request,
            },
        )
    }

    fn apps_look(&self) -> Fix {
        let link = format!("{}/admin/application_settings/general", self.base());
        Fix {
            link: link.clone(),
            steps: vec![
                format!("Only an administrator can read this: open {link}."),
                "Expand Account and limit and look at Allow users to register any application."
                    .to_owned(),
            ],
            request: self.request(
                Who::Admin,
                &format!(
                    "Please tell me whether {} lets users register OAuth applications ({link}).",
                    self.host
                ),
            ),
        }
    }

    fn protection_fix(&self, person: Option<&RepoFacts>) -> (Who, Fix) {
        let who = self.owner_fixer(person);
        let (link, steps) = match self.kind {
            Kind::GitHub => {
                let link = format!("{}/settings/rules/new?target=branch", self.repo_url());
                (
                    link.clone(),
                    vec![
                        format!("Open {link}."),
                        "Name the ruleset, set Enforcement to Active.".to_owned(),
                        "Under Target branches add the default branch.".to_owned(),
                        "Tick Require a pull request before merging, then press Create.".to_owned(),
                    ],
                )
            }
            Kind::GitLab => {
                let link = format!("{}/-/settings/repository", self.repo_url());
                (
                    link.clone(),
                    vec![
                        format!("Open {link}."),
                        "Expand Protected branches and press Add protected branch.".to_owned(),
                        "Pick the default branch; set Allowed to merge to Maintainers.".to_owned(),
                        "Set Allowed to push and merge to No one, then press Protect.".to_owned(),
                    ],
                )
            }
        };
        let request = self.request(
            who,
            &format!(
                "The default branch of {} is not protected. Please protect it at {link} (require a pull request before merging).",
                self.path
            ),
        );
        (
            who,
            Fix {
                link,
                steps,
                request,
            },
        )
    }

    fn protection_look(&self, person: Option<&RepoFacts>) -> Fix {
        let (_, mut fix) = self.protection_fix(person);
        fix.steps.insert(
            0,
            "purlis could not read the protection (the forge shows it to admins only): look at it yourself.".to_owned(),
        );
        fix
    }

    fn ci_fix(&self, person: Option<&RepoFacts>) -> (Who, Fix) {
        let who = self.owner_fixer(person);
        let (link, steps) = match self.kind {
            Kind::GitHub => {
                let link = format!("{}/settings/actions", self.repo_url());
                (
                    link.clone(),
                    vec![
                        format!("Open {link}."),
                        "Under Workflow permissions choose Read and write permissions.".to_owned(),
                        "Press Save. (A workflow can instead ask for contents: write in its own permissions block.)".to_owned(),
                    ],
                )
            }
            Kind::GitLab => {
                let link = format!("{}/-/settings/ci_cd", self.repo_url());
                (
                    link.clone(),
                    vec![
                        format!("Open {link}."),
                        "Expand Job token permissions.".to_owned(),
                        "Tick Allow Git push requests to the repository.".to_owned(),
                        "Press Save changes.".to_owned(),
                    ],
                )
            }
        };
        let request = self.request(
            who,
            &format!(
                "CI's token may only read {}. Please let it push at {link}.",
                self.path
            ),
        );
        (
            who,
            Fix {
                link,
                steps,
                request,
            },
        )
    }

    fn ci_look(&self) -> Fix {
        let (_, fix) = self.ci_fix(None);
        Fix {
            steps: std::iter::once(
                "The forge shows this to the repo's admins only: ask one, or open the page yourself.".to_owned(),
            )
            .chain(fix.steps)
            .collect(),
            ..fix
        }
    }
}

/// The error a probe turns into an unknown's reason.
pub(super) fn why(e: &ForgeError) -> String {
    one_line(e.said())
}

#[cfg(test)]
mod tests;
