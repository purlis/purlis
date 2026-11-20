// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which forge requests are held acts** (ADR 0128 D8, D3d): the proxy reads a chat's request,
//! and where it is a merge, a tag, a release or a fork it does not forward it. It answers the chat
//! with the one sentence of [`crate::heldact::said`] and raises an ask from what the request
//! named.
//!
//! **Nothing is parsed from a harness.** `gh pr merge 12` and `glab mr merge 12` are held because
//! of the request each one sends, which the proxy already had to understand to refuse it: `glab`'s
//! is `PUT /projects/:id/merge_requests/12/merge`, and `gh`'s is GraphQL's `mergePullRequest`,
//! which the proxy's GraphQL reading resolves to a recorded repository and a number and hands to
//! [`Requested::merge_of`].
//!
//! A request here is **only what the chat asked**. It carries names and never a fact: the commit a
//! tag lands on, whether it is on the default branch, the merge's head and base are all read from
//! the forge by the app ([`crate::heldact::prepare`]) when the ask is made, and read again at each
//! press.

use super::TagName;
use crate::forge::Kind;
use crate::mergehold::{Repo, Request};

/// **What a chat asked for**, in the words the request used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Requested {
    /// A merge of one request. It is the merge hold's ([`crate::mergehold`]): the app reads its
    /// head, base and checks, and merges at the head the person viewed.
    Merge(Request),
    /// A tag of `rev` (a commit, a branch's name or any name the forge resolves) under `name`.
    Tag {
        repo: Repo,
        name: TagName,
        rev: String,
    },
    /// A release of `tag`, whose commit is `rev` where the tag does not exist yet.
    Release {
        repo: Repo,
        tag: TagName,
        rev: Option<String>,
        title: String,
        notes: String,
        draft: bool,
        prerelease: bool,
    },
    /// A fork of `source` into the person's own account.
    Fork { source: Repo },
    /// A push of `tip` to `branch` that the tree diff found to change CI files. The proxy makes
    /// it from the push it read ([`crate::heldact::check_push`]); [`recognise`] never does.
    CiPush {
        repo: Repo,
        branch: crate::mergehold::Base,
        tip: crate::mergehold::HeadSha,
    },
}

impl Requested {
    /// The merge of request `number` of `repo`: what the GraphQL reading hands over for a
    /// `mergePullRequest` or an `enablePullRequestAutoMerge` it resolved to a recorded repository.
    pub fn merge_of(repo: Repo, number: u64) -> Requested {
        Requested::Merge(Request {
            repo: Some(repo),
            number,
        })
    }

    /// The word for the kind of act, as the ask and the chat's sentence use it.
    pub fn word(&self) -> &'static str {
        match self {
            Requested::Merge(_) => "merge",
            Requested::Tag { .. } => "tag",
            Requested::Release { .. } => "release",
            Requested::Fork { .. } => "fork",
            Requested::CiPush { .. } => "CI-file push",
        }
    }
}

/// Why a request that looks like a held act is refused, and not held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Why {
    /// A tag that exists is never moved: an update of a ref under `refs/tags/`.
    TagMoved,
    /// A tag is never deleted by a chat or by an ask.
    TagDeleted,
    /// A fork to another organization or namespace is refused, whoever asks.
    ForkElsewhere,
    /// The repository is not one the person recorded for this chat (D4).
    NotRecorded,
    /// The request names something purlis cannot read as one act, in these words.
    Unreadable(&'static str),
}

/// What the proxy does with one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recognised {
    /// A held act: do not forward it, ask the person.
    Hold(Requested),
    /// Refused, with no ask.
    Refuse(Why),
    /// Not a held act: the proxy's ordinary rules decide.
    Other,
}

/// **The repos the person recorded for a chat** (D4), as the recogniser asks it.
pub trait Recorded {
    /// The recorded repo `id` names on a forge of `kind`: GitHub's `owner/name`, and on GitLab a
    /// project's path or its numeric id. `None` where `id` is not a recorded repo.
    fn resolve(&self, kind: Kind, id: &str) -> Option<Repo>;
    /// Whether `repo` is a recorded repo.
    fn holds(&self, repo: &Repo) -> bool;
}

/// A request as the proxy's reading gives it: the method, the path's segments (each decoded once)
/// and the query, with the body where the request has one.
#[derive(Debug, Clone, Copy)]
pub struct Call<'a> {
    pub method: &'a str,
    pub segments: &'a [String],
    pub query: &'a [(String, String)],
    pub body: &'a serde_json::Value,
}

impl Call<'_> {
    /// A parameter, from the body (a string, a bool or a number) or the query.
    fn param(&self, name: &str) -> Option<String> {
        if let Some(value) = self.body.get(name) {
            return match value {
                serde_json::Value::String(text) => Some(text.clone()),
                serde_json::Value::Bool(flag) => Some(flag.to_string()),
                serde_json::Value::Number(number) => Some(number.to_string()),
                _ => None,
            };
        }
        self.query
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }

    fn flag(&self, name: &str) -> bool {
        self.param(name).as_deref() == Some("true")
    }

    fn named(&self, name: &str) -> bool {
        self.body.get(name).is_some_and(|value| !value.is_null())
            || self.query.iter().any(|(key, _)| key == name)
    }
}

/// **Reads one request**: a held act, a refusal, or none of them.
pub fn recognise(kind: Kind, call: &Call<'_>, recorded: &dyn Recorded) -> Recognised {
    let segments: Vec<&str> = call.segments.iter().map(String::as_str).collect();
    let (repo_id, rest): (String, &[&str]) = match (kind, &segments[..]) {
        (Kind::GitHub, ["api", "v3", "repos", owner, name, rest @ ..])
        | (Kind::GitHub, ["repos", owner, name, rest @ ..]) => (format!("{owner}/{name}"), rest),
        (Kind::GitLab, ["api", "v4", "projects", id, rest @ ..]) => ((*id).to_owned(), rest),
        _ => return Recognised::Other,
    };
    let Some(verb) = verb(kind, call, rest) else {
        return Recognised::Other;
    };
    let Some(repo) = recorded.resolve(kind, &repo_id) else {
        return Recognised::Refuse(Why::NotRecorded);
    };
    verb.into(repo, call)
}

/// What a request does, before its repo is known.
enum Verb {
    Merge(u64),
    Tag,
    Release,
    Fork,
    TagMoved,
    TagDeleted,
    Unreadable(&'static str),
}

impl Verb {
    fn into(self, repo: Repo, call: &Call<'_>) -> Recognised {
        match self {
            Verb::Merge(number) => Recognised::Hold(Requested::merge_of(repo, number)),
            Verb::Tag => tag(repo, call),
            Verb::Release => release(repo, call),
            Verb::Fork => fork(repo, call),
            Verb::TagMoved => Recognised::Refuse(Why::TagMoved),
            Verb::TagDeleted => Recognised::Refuse(Why::TagDeleted),
            Verb::Unreadable(why) => Recognised::Refuse(Why::Unreadable(why)),
        }
    }
}

fn verb(kind: Kind, call: &Call<'_>, rest: &[&str]) -> Option<Verb> {
    match (kind, call.method.to_ascii_uppercase().as_str(), rest) {
        // Merges.
        (Kind::GitHub, "PUT", ["pulls", number, "merge"])
        | (Kind::GitLab, "PUT", ["merge_requests", number, "merge"]) => {
            Some(match number.parse::<u64>() {
                Ok(n) if n > 0 && number.bytes().all(|b| b.is_ascii_digit()) => Verb::Merge(n),
                _ => Verb::Unreadable("a merge whose request number is not a number"),
            })
        }
        // GitHub: a tag is a ref.
        (Kind::GitHub, "POST", ["git", "refs"]) => {
            let name = call.param("ref")?;
            name.starts_with("refs/tags/").then_some(Verb::Tag)
        }
        (Kind::GitHub, "PATCH" | "PUT", ["git", "refs", "tags", ..]) => Some(Verb::TagMoved),
        (Kind::GitHub, "DELETE", ["git", "refs", "tags", ..]) => Some(Verb::TagDeleted),
        (Kind::GitHub, "POST", ["releases"]) => Some(Verb::Release),
        (Kind::GitHub, "POST", ["forks"]) => Some(Verb::Fork),
        // GitLab.
        (Kind::GitLab, "POST", ["repository", "tags"]) => Some(Verb::Tag),
        (Kind::GitLab, "DELETE", ["repository", "tags", ..]) => Some(Verb::TagDeleted),
        (Kind::GitLab, "POST", ["releases"]) => Some(Verb::Release),
        (Kind::GitLab, "POST", ["fork"]) => Some(Verb::Fork),
        _ => None,
    }
}

fn tag(repo: Repo, call: &Call<'_>) -> Recognised {
    let (name, rev) = match (call.param("tag_name"), call.param("ref")) {
        // GitLab: `tag_name` and `ref`.
        (Some(name), Some(rev)) => (name, rev),
        // GitHub: `ref` is `refs/tags/<name>` and `sha` is the commit.
        (None, Some(full)) => match (TagName::from_ref(&full), call.param("sha")) {
            (Some(Ok(name)), Some(sha)) => (name.as_str().to_owned(), sha),
            (Some(Err(_)), _) => {
                return Recognised::Refuse(Why::Unreadable("a tag name git refuses"));
            }
            _ => return Recognised::Refuse(Why::Unreadable("a tag with no commit named")),
        },
        _ => return Recognised::Refuse(Why::Unreadable("a tag with no name or no commit")),
    };
    match TagName::try_from(name) {
        Ok(name) => Recognised::Hold(Requested::Tag { repo, name, rev }),
        Err(_) => Recognised::Refuse(Why::Unreadable("a tag name git refuses")),
    }
}

fn release(repo: Repo, call: &Call<'_>) -> Recognised {
    let Some(tag) = call.param("tag_name") else {
        return Recognised::Refuse(Why::Unreadable("a release with no tag"));
    };
    let Ok(tag) = TagName::try_from(tag) else {
        return Recognised::Refuse(Why::Unreadable("a tag name git refuses"));
    };
    // GitHub's `target_commitish` and GitLab's `ref`: where the tag is made, if it is not there.
    let rev = call
        .param("target_commitish")
        .or_else(|| call.param("ref"))
        .filter(|rev| !rev.is_empty());
    Recognised::Hold(Requested::Release {
        repo,
        tag,
        rev,
        title: call.param("name").unwrap_or_default(),
        notes: call
            .param("body")
            .or_else(|| call.param("description"))
            .unwrap_or_default(),
        draft: call.flag("draft"),
        prerelease: call.flag("prerelease"),
    })
}

fn fork(repo: Repo, call: &Call<'_>) -> Recognised {
    // The person's own account is the only destination, and a chat cannot name one: any
    // organization or namespace in the request is another place.
    let elsewhere = [
        "organization",
        "namespace",
        "namespace_id",
        "namespace_path",
    ];
    if elsewhere.iter().any(|name| call.named(name)) {
        return Recognised::Refuse(Why::ForkElsewhere);
    }
    Recognised::Hold(Requested::Fork { source: repo })
}
