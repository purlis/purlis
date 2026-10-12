// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! [`Making`] on GitLab (#1819): the owners an account can make a project under, the project
//! itself, and taking back the one a run made. REST v4; the `api` scope covers all three.
//!
//! - **Owners**: `GET user`, then `GET groups?min_access_level=30`, the groups where the account
//!   is at least a Developer, which is the least role GitLab lets make a project in a group by
//!   default. A group that asks for more refuses the create, and the refusal is said.
//! - **Create**: `POST projects`, in a group by its `namespace_id`, with
//!   `initialize_with_readme: false`, so the project is empty.
//! - **Delete**: `DELETE projects/{id}`, by the id the create answered: GitLab's ids never move
//!   to another project, so only the one this run made can be named. A project GitLab keeps for
//!   a few days before it deletes it ("delayed deletion") is out of the way at once: GitLab
//!   renames it and frees its path.

use serde_json::Value;

use super::super::backend::{Caller, MadeRepo, Making, NewRepo, OwnerKind, RepoOwner, Visibility};
use super::super::transport::{Call, Field, Method, Reply};
use super::super::{Failure, ForgeError, ForgeRef, Kind, listed_id, listed_str, quote};
use super::GitLab;

/// Whether `reply` is GitLab saying there is nothing at the path asked.
fn not_there(reply: &Reply) -> bool {
    reply.status == Some(404) || reply.both().contains("HTTP 404")
}

impl Making for GitLab {
    fn owners_to_create_under(&self, caller: &Caller) -> Result<Vec<RepoOwner>, ForgeError> {
        let me = self
            .0
            .strict(caller, "user", "reading who this GitLab account is")?;
        let login = listed_str(&me, "username");
        if login.is_empty() {
            return Err(ForgeError::new(
                "GitLab answered who this account is without a username".to_string(),
            ));
        }
        let title = Some(listed_str(&me, "name"))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| login.clone());
        let mut owners = vec![RepoOwner {
            name: login,
            title,
            kind: OwnerKind::You,
            id: listed_id(&me),
        }];
        let mut page = 1;
        loop {
            let path = format!("groups?min_access_level=30&per_page=100&page={page}");
            let listed = self.0.strict(
                caller,
                &path,
                "listing the GitLab groups this account can make a project in",
            )?;
            let groups = listed.as_array().cloned().unwrap_or_default();
            for group in &groups {
                let name = listed_str(group, "full_path");
                let id = listed_id(group);
                if name.is_empty() || id.is_none() {
                    continue;
                }
                let title = Some(listed_str(group, "full_name"))
                    .filter(|title| !title.is_empty())
                    .unwrap_or_else(|| name.clone());
                owners.push(RepoOwner {
                    name,
                    title,
                    kind: OwnerKind::Organisation,
                    id,
                });
            }
            if groups.len() < 100 {
                break;
            }
            page += 1;
        }
        Ok(owners)
    }

    fn create_repo(&self, caller: &Caller, new: &NewRepo) -> Result<MadeRepo, ForgeError> {
        let visibility = if new.visibility == Visibility::Public {
            "public"
        } else {
            "private"
        };
        let mut fields = vec![
            Field::text("name", &new.name),
            Field::text("path", &new.name),
            Field::text("visibility", visibility),
            Field::typed("initialize_with_readme", "false"),
        ];
        let doing = format!(
            "creating the project {}/{} on GitLab",
            new.owner.name, new.name
        );
        if new.owner.kind == OwnerKind::Organisation {
            // A group is named by its namespace id, which only a number is: anything else is not
            // the id GitLab listed, and nothing is sent.
            let Some(ForgeRef(id)) = new
                .owner
                .id
                .as_ref()
                .filter(|id| !id.0.is_empty() && id.0.bytes().all(|b| b.is_ascii_digit()))
            else {
                return Err(ForgeError::new(format!(
                    "{doing}: purlis has no GitLab id for the group {}",
                    new.owner.name
                )));
            };
            fields.push(Field::typed("namespace_id", id));
        }
        let call = Call::write(Method::Post, "projects", fields);
        let reply = self
            .0
            .send(caller, &call)
            .map_err(|no| no.error(format!("{doing}: {}", no.said())))?;
        if !reply.ok() {
            // GitLab says a name or path in use with a 400, where GitHub says it with a 422.
            let failure =
                if reply.status == Some(400) && reply.both().contains("already been taken") {
                    Failure::Conflict
                } else {
                    reply.failure()
                };
            return Err(ForgeError::of(
                failure,
                format!("{doing} failed: {}", reply.said(Kind::GitLab)),
            ));
        }
        let answer: Value = serde_json::from_str(&reply.out).map_err(|e| {
            ForgeError::new(format!("{doing}: GitLab answered malformed JSON: {e}"))
        })?;
        made_of(&answer, &doing)
    }

    fn delete_created_repo(&self, caller: &Caller, made: &MadeRepo) -> Result<(), ForgeError> {
        let doing = format!("deleting the project {} purlis made on GitLab", made.path());
        let call = Call::write(
            Method::Delete,
            format!("projects/{}", quote(&made.id().0)),
            Vec::new(),
        );
        match self.0.send(caller, &call) {
            Ok(reply) if reply.ok() || not_there(&reply) => Ok(()),
            Ok(reply) => Err(ForgeError::of(
                reply.failure(),
                format!("{doing} failed: {}", reply.said(Kind::GitLab)),
            )),
            Err(no) => Err(no.error(no.said().to_string())),
        }
    }
}

/// The made project in GitLab's answer to a create, or why the answer is not one.
fn made_of(answer: &Value, doing: &str) -> Result<MadeRepo, ForgeError> {
    let path = listed_str(answer, "path_with_namespace");
    let clone_url = listed_str(answer, "http_url_to_repo");
    match listed_id(answer) {
        Some(ForgeRef(id))
            if !path.is_empty()
                && clone_url.starts_with("https://")
                && id.bytes().all(|b| b.is_ascii_digit()) =>
        {
            Ok(MadeRepo::answered(
                ForgeRef(id),
                path,
                listed_str(answer, "web_url"),
                clone_url,
            ))
        }
        _ => Err(ForgeError::new(format!(
            "{doing}: GitLab's answer names no project with an id and an HTTPS clone URL"
        ))),
    }
}
