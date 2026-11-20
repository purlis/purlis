// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! GitLab's reads for a project's readiness ([`crate::forge::readiness`]).
//!
//! - `GET projects/{path}`: `default_branch`, `namespace.kind`, the account's access level on
//!   the project (`permissions.project_access`) and on its group (`permissions.group_access`),
//!   whichever is higher, and `ci_push_repository_for_job_token_allowed`, the job-token push
//!   permission. GitLab has no organization approval of an OAuth app, so the org access is not
//!   applicable.
//! - `GET user`: `is_admin`, which only decides who can fix a blocker.
//! - `GET application/settings`: `user_oauth_applications`, which only an instance
//!   administrator can read; anyone else's refusal is unknown, never "allowed".

use serde_json::Value;

use super::GitLab;
use crate::forge::backend::Caller;
use crate::forge::readiness::{Fact, OrgAccess, Probe, RepoFacts, RepoRead, Role, why};
use crate::forge::transport::Call;
use crate::forge::{LIST_TIMEOUT, quote};

/// GitLab's access levels.
const DEVELOPER: u64 = 30;
const MAINTAINER: u64 = 40;
const OWNER: u64 = 50;

impl Probe for GitLab {
    fn repo(&self, caller: &Caller, path: &str) -> RepoRead {
        let api = format!("projects/{}", quote(path));
        let doing = format!("reading {path}");
        let project = match self.0.ask(caller, &Call::get(&api, LIST_TIMEOUT), &doing) {
            Ok(project) => project,
            Err(e) => return RepoRead::Unknown(why(&e)),
        };
        match facts_of(&project) {
            Ok(mut facts) => {
                facts.instance_admin = self
                    .0
                    .best_effort(caller, "user")
                    .is_some_and(|user| user["is_admin"] == Value::Bool(true));
                RepoRead::Read(facts)
            }
            Err(said) => RepoRead::Unknown(said),
        }
    }

    fn user_apps(&self, caller: &Caller) -> Fact<bool> {
        let doing = "reading the instance's application settings";
        match self.0.ask(
            caller,
            &Call::get("application/settings", LIST_TIMEOUT),
            doing,
        ) {
            Err(e) => Fact::Unknown(format!(
                "only an instance administrator can read it ({})",
                why(&e)
            )),
            Ok(settings) => settings["user_oauth_applications"].as_bool().map_or_else(
                || Fact::unknown("GitLab did not say whether users may register applications"),
                Fact::Known,
            ),
        }
    }

    fn ci_token(&self, caller: &Caller, path: &str) -> Fact<bool> {
        let api = format!("projects/{}", quote(path));
        let doing = format!("reading the CI job token settings of {path}");
        match self.0.ask(caller, &Call::get(&api, LIST_TIMEOUT), &doing) {
            Err(e) => Fact::Unknown(why(&e)),
            Ok(project) => project["ci_push_repository_for_job_token_allowed"]
                .as_bool()
                .map_or_else(
                    || {
                        Fact::unknown(
                            "GitLab did not show the job token push setting (older than 17.2, or not a maintainer)",
                        )
                    },
                    Fact::Known,
                ),
        }
    }
}

/// What a project's answer says, or why it says too little to judge by.
fn facts_of(project: &Value) -> Result<RepoFacts, String> {
    let said = |what: &str| format!("GitLab's answer for the project has no {what}");
    let default_branch = project["default_branch"]
        .as_str()
        .filter(|b| !b.is_empty())
        .ok_or_else(|| said("default branch"))?;
    let namespace = &project["namespace"];
    let owner = namespace["full_path"]
        .as_str()
        .ok_or_else(|| said("namespace"))?;
    let owner_is_group = match namespace["kind"].as_str() {
        Some("group") => true,
        Some("user") => false,
        _ => return Err(said("namespace kind")),
    };
    // A project the account has no membership in still shows `permissions` (both levels null).
    // No `permissions` at all says nothing about the account, so it is not a level of 0.
    if !project["permissions"].is_object() {
        return Err("GitLab did not show this account's permissions on the project".to_owned());
    }
    let level = |from: &str| project["permissions"][from]["access_level"].as_u64();
    let best = level("project_access").max(level("group_access"));
    // No membership at all reads a public project and may do nothing else.
    let level = best.unwrap_or(0);
    let role = if level >= MAINTAINER {
        Role::Admin
    } else if level >= DEVELOPER {
        Role::Write
    } else {
        Role::Read
    };
    Ok(RepoFacts {
        owner: owner.to_owned(),
        owner_is_group,
        default_branch: default_branch.to_owned(),
        role,
        org_owner: level >= OWNER,
        instance_admin: false,
        org_access: OrgAccess::NotApplicable,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn project(project_level: Option<u64>, group_level: Option<u64>) -> Value {
        json!({"default_branch": "main",
               "namespace": {"kind": "group", "full_path": "acme"},
               "permissions": {
                   "project_access": project_level.map(|l| json!({"access_level": l})),
                   "group_access": group_level.map(|l| json!({"access_level": l}))}})
    }

    #[test]
    fn a_developer_may_push_and_a_reporter_may_not() {
        assert_eq!(
            facts_of(&project(Some(30), None)).unwrap().role,
            Role::Write
        );
        assert_eq!(facts_of(&project(Some(20), None)).unwrap().role, Role::Read);
        assert_eq!(facts_of(&project(Some(10), None)).unwrap().role, Role::Read);
    }

    #[test]
    fn the_higher_of_the_project_and_group_access_counts() {
        let facts = facts_of(&project(Some(20), Some(40))).unwrap();
        assert_eq!(facts.role, Role::Admin);
        assert!(!facts.org_owner);
        assert!(facts_of(&project(None, Some(50))).unwrap().org_owner);
    }

    #[test]
    fn no_membership_is_read_only() {
        assert_eq!(facts_of(&project(None, None)).unwrap().role, Role::Read);
    }

    #[test]
    fn an_answer_without_permissions_is_not_judged_as_no_membership() {
        let mut answer = project(Some(40), None);
        answer["permissions"] = Value::Null;
        assert!(facts_of(&answer).is_err());
        answer.as_object_mut().unwrap().remove("permissions");
        assert!(facts_of(&answer).is_err());
    }

    #[test]
    fn an_answer_without_a_default_branch_is_not_judged() {
        let mut answer = project(Some(40), None);
        answer["default_branch"] = Value::Null;
        assert!(facts_of(&answer).is_err());
    }
}
