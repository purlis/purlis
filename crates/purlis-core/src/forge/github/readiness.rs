// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! GitHub's reads for a project's readiness ([`crate::forge::readiness`]).
//!
//! - `GET repos/{o}/{r}`: `private`, `owner.type`, `default_branch` and the account's
//!   `permissions`. A 403 that names "OAuth App access restrictions" is the organization
//!   refusing purlis's sign-in: GitHub's own words are the only signal there is, since nothing
//!   in the REST API lists an organization's approved apps to a member. A private repo that
//!   reads is proof of approval; a public one proves nothing (an unapproved app reads public
//!   data), so it is unknown.
//! - `GET user/memberships/orgs/{org}`: whether the account owns the organization (`role` is
//!   `admin`), and `GET user`: `site_admin`. Both only decide *who* can fix a blocker, so a
//!   failed read is "not an owner", the stricter answer.
//! - `GET repos/{o}/{r}/actions/permissions/workflow`: `default_workflow_permissions`, which
//!   only the repo's admins can read.

use serde_json::Value;

use super::GitHub;
use crate::forge::backend::{Caller, read_reply};
use crate::forge::readiness::{Fact, OrgAccess, Probe, RepoFacts, RepoRead, Role, one_line, why};
use crate::forge::transport::{Call, Reply};
use crate::forge::{LIST_TIMEOUT, quote};

/// The sentence GitHub's refusal carries, lower-cased for the comparison.
const RESTRICTION: &str = "oauth app access restrictions";

fn restricted(reply: &Reply) -> bool {
    reply.both().to_ascii_lowercase().contains(RESTRICTION)
}

impl Probe for GitHub {
    fn repo(&self, caller: &Caller, path: &str) -> RepoRead {
        let (owner, name) = super::owner_name(path);
        let api = format!("repos/{}/{}", quote(owner), quote(name));
        let reply = match self.0.send(caller, &Call::get(&api, LIST_TIMEOUT)) {
            Ok(reply) => reply,
            Err(no) => return RepoRead::Unknown(one_line(no.said())),
        };
        if !reply.ok() {
            let said = one_line(&reply.said(self.0.kind()));
            return if restricted(&reply) {
                RepoRead::Restricted(said)
            } else {
                // A 404 is also what GitHub answers for a private repo the account may not see.
                RepoRead::Unknown(said)
            };
        }
        let repo = match read_reply(&reply) {
            Ok(repo) => repo,
            Err(e) => return RepoRead::Unknown(format!("GitHub answered malformed JSON: {e}")),
        };
        facts_of(&repo).map_or_else(RepoRead::Unknown, |mut facts| {
            if facts.owner_is_group {
                facts.org_owner = self.owns(caller, &facts.owner);
            }
            facts.instance_admin = self
                .0
                .best_effort(caller, "user")
                .is_some_and(|user| user["site_admin"] == Value::Bool(true));
            RepoRead::Read(facts)
        })
    }

    fn user_apps(&self, _caller: &Caller) -> Fact<bool> {
        Fact::unknown("GitHub has no such setting")
    }

    fn ci_token(&self, caller: &Caller, path: &str) -> Fact<bool> {
        let (owner, name) = super::owner_name(path);
        let api = format!(
            "repos/{}/{}/actions/permissions/workflow",
            quote(owner),
            quote(name)
        );
        let doing = format!("reading the workflow permissions of {path}");
        match self.0.ask(caller, &Call::get(&api, LIST_TIMEOUT), &doing) {
            Err(e) => Fact::Unknown(why(&e)),
            Ok(answer) => match answer["default_workflow_permissions"].as_str() {
                Some("write") => Fact::Known(true),
                Some("read") => Fact::Known(false),
                _ => Fact::unknown("GitHub did not say what a workflow token may do"),
            },
        }
    }
}

impl GitHub {
    /// Whether the account owns the organization `org`.
    fn owns(&self, caller: &Caller, org: &str) -> bool {
        self.0
            .best_effort(caller, &format!("user/memberships/orgs/{}", quote(org)))
            .is_some_and(|m| m["role"] == "admin" && m["state"] == "active")
    }
}

/// What a repo's answer says, or why it says too little to judge by.
fn facts_of(repo: &Value) -> Result<RepoFacts, String> {
    let said = |what: &str| format!("GitHub's answer for the repo has no {what}");
    let private = repo["private"].as_bool().ok_or_else(|| said("private"))?;
    let owner = repo["owner"]["login"]
        .as_str()
        .ok_or_else(|| said("owner"))?;
    let owner_is_group = match repo["owner"]["type"].as_str() {
        Some("Organization") => true,
        Some("User") => false,
        _ => return Err(said("owner type")),
    };
    let default_branch = repo["default_branch"]
        .as_str()
        .filter(|b| !b.is_empty())
        .ok_or_else(|| said("default branch"))?;
    let permissions = &repo["permissions"];
    let flag = |name: &str| permissions[name].as_bool();
    let role = match (flag("admin"), flag("push"), flag("pull")) {
        (Some(true), _, _) => Role::Admin,
        (_, Some(true), _) => Role::Write,
        (Some(false), Some(false), Some(_)) | (None, Some(false), Some(_)) => Role::Read,
        _ => return Err("GitHub did not show this account's permissions on the repo".to_owned()),
    };
    let org_access = match (owner_is_group, private) {
        (false, _) => OrgAccess::NotApplicable,
        (true, true) => OrgAccess::Approved,
        (true, false) => OrgAccess::Unproven,
    };
    Ok(RepoFacts {
        owner: owner.to_owned(),
        owner_is_group,
        default_branch: default_branch.to_owned(),
        role,
        org_owner: false,
        instance_admin: false,
        org_access,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn repo(private: bool, kind: &str, permissions: Value) -> Value {
        json!({"private": private, "owner": {"login": "acme", "type": kind},
               "default_branch": "main", "permissions": permissions})
    }

    #[test]
    fn a_private_organization_repo_that_reads_proves_the_approval() {
        let facts = facts_of(&repo(
            true,
            "Organization",
            json!({"admin": false, "push": true, "pull": true}),
        ))
        .unwrap();
        assert_eq!(facts.org_access, OrgAccess::Approved);
        assert_eq!(facts.role, Role::Write);
    }

    #[test]
    fn a_public_organization_repo_proves_nothing_about_the_approval() {
        let facts = facts_of(&repo(
            false,
            "Organization",
            json!({"admin": false, "push": true, "pull": true}),
        ))
        .unwrap();
        assert_eq!(facts.org_access, OrgAccess::Unproven);
    }

    #[test]
    fn a_personal_repo_has_no_approval_to_ask() {
        let facts = facts_of(&repo(
            true,
            "User",
            json!({"admin": true, "push": true, "pull": true}),
        ))
        .unwrap();
        assert_eq!(facts.org_access, OrgAccess::NotApplicable);
        assert_eq!(facts.role, Role::Admin);
    }

    #[test]
    fn a_read_only_account_is_read() {
        let facts = facts_of(&repo(
            true,
            "Organization",
            json!({"admin": false, "push": false, "pull": true}),
        ))
        .unwrap();
        assert_eq!(facts.role, Role::Read);
    }

    #[test]
    fn an_answer_without_permissions_is_not_judged() {
        assert!(facts_of(&repo(true, "Organization", Value::Null)).is_err());
    }

    #[test]
    fn an_answer_without_a_default_branch_is_not_judged() {
        let mut answer = repo(
            true,
            "User",
            json!({"admin": true, "push": true, "pull": true}),
        );
        answer["default_branch"] = Value::Null;
        assert!(facts_of(&answer).is_err());
    }

    #[test]
    fn githubs_restriction_sentence_is_recognised_in_either_stream() {
        let said = "Although you appear to have the correct authorization credentials, the `acme` \
                    organization has enabled OAuth App access restrictions, meaning that data access \
                    to third-parties is limited.";
        assert!(restricted(&Reply::of(1, said.to_owned(), String::new())));
        assert!(restricted(&Reply::of(
            1,
            String::new(),
            format!("gh: {said} (HTTP 403)")
        )));
        assert!(!restricted(&Reply::of(
            1,
            String::new(),
            "gh: Not Found (HTTP 404)".to_owned()
        )));
    }
}
