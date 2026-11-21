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
    /// The branch-merge endpoint (`POST …/merges`), which merges with no request to hold.
    MergesWithoutRequest,
    /// A change to a release that exists, or a file added to one: never a chat's.
    ReleaseEdit,
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
    /// Not a held act: the proxy's allowlist decides, and **a request it does not list is
    /// refused**. `Other` is never permission.
    Other,
}

/// **The repos the person recorded for a chat** (D4), as the recogniser asks it.
pub trait Recorded: Send + Sync {
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
    // The proxy decodes each segment once, so `git/refs/tags%2Fv1` arrives as the segments `git`,
    // `refs` and `tags/v1`: read as the path it is, every segment split at its slashes. (A
    // GitLab project's id is the one segment that keeps its slash, and is taken before this.)
    let flat = |segments: &[String]| -> Vec<String> {
        segments
            .iter()
            .flat_map(|segment| segment.split('/').map(str::to_owned))
            .collect()
    };
    let (repo_id, rest): (String, Vec<String>) = match kind {
        Kind::GitHub => {
            let all = flat(call.segments);
            let all: Vec<&str> = all.iter().map(String::as_str).collect();
            match &all[..] {
                ["api", "v3", "repos", owner, name, rest @ ..]
                | ["repos", owner, name, rest @ ..] => (
                    format!("{owner}/{name}"),
                    rest.iter().map(|part| (*part).to_owned()).collect(),
                ),
                _ => return Recognised::Other,
            }
        }
        Kind::GitLab => match call.segments {
            [api, v4, projects, id, rest @ ..]
                if api == "api" && v4 == "v4" && projects == "projects" =>
            {
                (id.clone(), flat(rest))
            }
            _ => return Recognised::Other,
        },
    };
    let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
    let rest = &rest[..];
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
    MergesWithoutRequest,
    ReleaseEdit,
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
            Verb::MergesWithoutRequest => Recognised::Refuse(Why::MergesWithoutRequest),
            Verb::ReleaseEdit => Recognised::Refuse(Why::ReleaseEdit),
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
        // The branch-merge endpoint merges with no pull request to hold.
        (Kind::GitHub, _, ["merges"]) => Some(Verb::MergesWithoutRequest),
        // A release is made once, on a press: changing, deleting or adding a file to one is not.
        (Kind::GitHub, "PATCH" | "PUT" | "DELETE", ["releases", ..])
        | (Kind::GitHub, "POST", ["releases", _, "assets", ..])
        | (Kind::GitHub, "POST", ["releases", "assets", ..]) => Some(Verb::ReleaseEdit),
        (Kind::GitLab, "PUT" | "PATCH" | "DELETE", ["releases", ..])
        | (Kind::GitLab, "POST", ["releases", _, ..]) => Some(Verb::ReleaseEdit),
        // A merge train merges the request in its turn.
        (Kind::GitLab, "POST" | "PUT", ["merge_trains", "merge_requests", iid]) => {
            Some(match iid.parse::<u64>() {
                Ok(n) if n > 0 && iid.bytes().all(|b| b.is_ascii_digit()) => Verb::Merge(n),
                _ => Verb::Unreadable("a merge train entry whose request number is not a number"),
            })
        }
        // GitLab.
        (Kind::GitLab, "POST", ["repository", "tags"]) => Some(Verb::Tag),
        (Kind::GitLab, "DELETE", ["repository", "tags", ..]) => Some(Verb::TagDeleted),
        (Kind::GitLab, "POST", ["releases"]) => Some(Verb::Release),
        (Kind::GitLab, "POST", ["fork"]) => Some(Verb::Fork),
        _ => None,
    }
}

/// **Whether `rev` is a name a forge may be asked to resolve**: a full object id (40 or 64 hex
/// digits) or a valid git ref name by `git check-ref-format`'s rules. A chat's words become part of
/// a request's path, so a `../` or a `@{` is never one.
pub fn valid_rev(rev: &str) -> bool {
    let hex = |n: usize| rev.len() == n && rev.bytes().all(|b| b.is_ascii_hexdigit());
    if hex(40) || hex(64) {
        return true;
    }
    if rev.is_empty()
        || rev.len() > 255
        || rev.starts_with('/')
        || rev.ends_with('/')
        || rev.ends_with('.')
        || rev.starts_with('-')
        || rev == "@"
        || rev.contains("..")
        || rev.contains("//")
        || rev.contains("@{")
        || rev
            .chars()
            .any(|c| c.is_control() || matches!(c, ' ' | '~' | '^' | ':' | '?' | '*' | '[' | '\\'))
    {
        return false;
    }
    rev.split('/')
        .all(|part| !part.starts_with('.') && !part.ends_with(".lock"))
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
    if !valid_rev(&rev) {
        return Recognised::Refuse(Why::Unreadable("a commit that is not an id or a ref name"));
    }
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
    if rev.as_deref().is_some_and(|rev| !valid_rev(rev)) {
        return Recognised::Refuse(Why::Unreadable("a commit that is not an id or a ref name"));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rev_is_an_object_id_or_a_valid_ref_name_and_nothing_else() {
        for ok in [
            "main",
            "feature/x",
            "v1.2.3",
            "HEAD",
            &"a".repeat(40),
            &"B".repeat(64),
        ] {
            assert!(valid_rev(ok), "{ok}");
        }
        for bad in [
            "",
            "../../../other/private/commits/main",
            "a/../b",
            "/main",
            "main/",
            "a//b",
            "main@{1}",
            "a b",
            "a~1",
            "a^",
            "a:b",
            "a?b",
            "a*b",
            "a[b",
            "a\\b",
            "a\u{7}b",
            "x.lock",
            ".hidden",
            "-rf",
            "end.",
        ] {
            assert!(!valid_rev(bad), "{bad:?}");
        }
    }

    #[test]
    fn a_tag_whose_commit_walks_the_api_is_refused_and_never_held() {
        struct Everything;
        impl Recorded for Everything {
            fn resolve(&self, _: Kind, id: &str) -> Option<Repo> {
                Repo::try_from(id).ok()
            }
            fn holds(&self, _: &Repo) -> bool {
                true
            }
        }
        let segments: Vec<String> = ["repos", "acme", "app", "git", "refs"]
            .map(str::to_owned)
            .to_vec();
        let body = serde_json::json!({
            "ref": "refs/tags/v1",
            "sha": "../../../other/private/commits/main"
        });
        let call = Call {
            method: "POST",
            segments: &segments,
            query: &[],
            body: &body,
        };
        assert_eq!(
            recognise(Kind::GitHub, &call, &Everything),
            Recognised::Refuse(Why::Unreadable("a commit that is not an id or a ref name"))
        );
    }
}
