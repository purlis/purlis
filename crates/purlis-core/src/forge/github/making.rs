// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! [`Making`] on GitHub (#1819): the owners an account can make a repo under, the repo itself,
//! and taking back the one a run made. REST `2022-11-28`.
//!
//! - **Owners**: `GET user`, then `GET user/orgs`, a page of 100 at a time. Listing the
//!   organisations needs `read:org`, which the token sign-in already asks for.
//! - **Create**: `POST user/repos` for the account, `POST orgs/{org}/repos` for an organisation,
//!   with `auto_init: false`, so the repo is empty and the project's first commit is its first.
//! - **Delete**: `GET repos/{owner}/{name}` first, and `DELETE` only when the repo there has the id
//!   the create answered. A repo renamed, moved or replaced since is not the one this run made,
//!   and is left alone.

use serde_json::Value;

use super::super::backend::{Caller, MadeRepo, Making, NewRepo, OwnerKind, RepoOwner, Visibility};
use super::super::transport::{Call, Field, Method};
use super::super::{ForgeError, ForgeRef, Kind, listed_id, listed_str, quote};
use super::GitHub;

/// `owner/name` with each part encoded for a REST path.
fn repo_path(path: &str) -> String {
    let (owner, name) = path.split_once('/').unwrap_or((path, ""));
    format!("repos/{}/{}", quote(owner), quote(name))
}

/// Whether `reply` is GitHub saying there is nothing at the path asked.
fn not_there(reply: &super::super::transport::Reply) -> bool {
    reply.status == Some(404) || reply.both().contains("HTTP 404")
}

impl Making for GitHub {
    fn owners_to_create_under(&self, caller: &Caller) -> Result<Vec<RepoOwner>, ForgeError> {
        let me = self
            .0
            .strict(caller, "user", "reading who this GitHub account is")?;
        let login = listed_str(&me, "login");
        if login.is_empty() {
            return Err(ForgeError::new(
                "GitHub answered who this account is without a login".to_string(),
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
            let path = format!("user/orgs?per_page=100&page={page}");
            let listed =
                self.0
                    .strict(caller, &path, "listing this GitHub account's organisations")?;
            let orgs = listed.as_array().cloned().unwrap_or_default();
            for org in &orgs {
                let name = listed_str(org, "login");
                if name.is_empty() {
                    continue;
                }
                owners.push(RepoOwner {
                    title: name.clone(),
                    name,
                    kind: OwnerKind::Organisation,
                    id: listed_id(org),
                });
            }
            if orgs.len() < 100 {
                break;
            }
            page += 1;
        }
        Ok(owners)
    }

    fn create_repo(&self, caller: &Caller, new: &NewRepo) -> Result<MadeRepo, ForgeError> {
        let path = match new.owner.kind {
            OwnerKind::You => "user/repos".to_string(),
            OwnerKind::Organisation => format!("orgs/{}/repos", quote(&new.owner.name)),
        };
        let private = if new.visibility == Visibility::Public {
            "false"
        } else {
            "true"
        };
        let doing = format!(
            "creating the repo {}/{} on GitHub",
            new.owner.name, new.name
        );
        let answer = self.0.ask(
            caller,
            &Call::write(
                Method::Post,
                path,
                vec![
                    Field::text("name", &new.name),
                    Field::typed("private", private),
                    Field::typed("auto_init", "false"),
                ],
            ),
            &doing,
        )?;
        made_of(&answer, &doing)
    }

    fn delete_created_repo(&self, caller: &Caller, made: &MadeRepo) -> Result<(), ForgeError> {
        let path = repo_path(made.path());
        let doing = format!("deleting the repo {} purlis made on GitHub", made.path());
        let Some(there) = self.0.found(caller, &path, &doing, not_there)? else {
            return Ok(());
        };
        if listed_id(&there).as_ref() != Some(made.id()) {
            return Err(ForgeError::of(
                super::super::Failure::Conflict,
                format!(
                    "{} on GitHub is no longer the repo purlis made a moment ago, so purlis \
                     deleted nothing there",
                    made.path()
                ),
            ));
        }
        let call = Call::write(Method::Delete, path, Vec::new());
        match self.0.send(caller, &call) {
            Ok(reply) if reply.ok() || not_there(&reply) => Ok(()),
            Ok(reply) => Err(ForgeError::of(
                reply.failure(),
                format!("{doing} failed: {}", reply.said(Kind::GitHub)),
            )),
            Err(no) => Err(no.error(no.said().to_string())),
        }
    }
}

/// The made repo in GitHub's answer to a create, or why the answer is not one.
fn made_of(answer: &Value, doing: &str) -> Result<MadeRepo, ForgeError> {
    let path = listed_str(answer, "full_name");
    let clone_url = listed_str(answer, "clone_url");
    match listed_id(answer) {
        Some(ForgeRef(id)) if !path.is_empty() && clone_url.starts_with("https://") => {
            Ok(MadeRepo::answered(
                ForgeRef(id),
                path,
                listed_str(answer, "html_url"),
                clone_url,
            ))
        }
        _ => Err(ForgeError::new(format!(
            "{doing}: GitHub's answer names no repo with an id and an HTTPS clone URL"
        ))),
    }
}
