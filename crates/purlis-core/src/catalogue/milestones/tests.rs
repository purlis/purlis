// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::json;

use super::*;
use crate::forge::transport::{Call, NoAnswer, Reply, Transport};
use crate::forge::{Forge, ForgeError};

/// A project with workspace `alpha` holding a clone for each `(name, origin)`.
fn project(clones: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    let ws = dir.path().join("workspaces/alpha");
    std::fs::create_dir_all(&ws).unwrap();
    for (name, origin) in clones {
        let clone = ws.join("repos").join(name);
        std::fs::create_dir_all(&clone).unwrap();
        assert!(crate::testgit::run(&clone, &["init", "-q"]).ok());
        if !origin.is_empty() {
            assert!(crate::testgit::run(&clone, &["remote", "add", "origin", origin]).ok());
        }
    }
    dir
}

/// A transport that answers every call alike, and keeps the paths it was asked.
struct Answers {
    reply: Result<Reply, NoAnswer>,
    asked: Mutex<Vec<String>>,
}

impl Transport for Answers {
    fn send(&self, _forge: &Forge, call: &Call) -> Result<Reply, NoAnswer> {
        self.asked.lock().unwrap().push(call.path().to_owned());
        self.reply.clone()
    }

    fn check_auth(&self, _forge: &Forge) -> Result<(), ForgeError> {
        Ok(())
    }
}

fn answering(reply: Result<Reply, NoAnswer>) -> Arc<Answers> {
    Arc::new(Answers {
        reply,
        asked: Mutex::new(Vec::new()),
    })
}

fn listed() -> Reply {
    Reply::of(
        0,
        json!([{
            "number": 7, "title": "M7", "description": "Ship it.", "due_on": null,
            "state": "open", "open_issues": 1, "closed_issues": 3,
            "html_url": "https://github.com/acme/api/milestone/7",
            "updated_at": "2026-11-10T09:00:00Z"
        }])
        .to_string(),
        String::new(),
    )
}

fn ask(root: &Path, args: Value, transport: &Arc<Answers>) -> Result<Value, Unanswered> {
    let transport = transport.clone();
    let backend_of = move |repo: &Repo| repo.forge.backend_over(transport.clone());
    read(
        root,
        "alpha",
        args.as_object().unwrap(),
        &Reach {
            caller: Caller::window().background(),
            backend_of: &backend_of,
        },
    )
}

fn asked(transport: &Answers) -> Vec<String> {
    transport.asked.lock().unwrap().clone()
}

#[test]
fn the_only_repo_s_open_milestones_are_read_off_its_own_origin() {
    let dir = project(&[("api", "https://github.com/acme/api.git")]);
    let transport = answering(Ok(listed()));
    let data = ask(dir.path(), json!({}), &transport).unwrap();
    assert_eq!(
        data,
        json!({
            "repo": "api",
            "path": "acme/api",
            "state": "open",
            "milestones": [{
                "number": 7, "title": "M7", "description": "Ship it.", "due_on": null,
                "state": "open", "open_issues": 1, "closed_issues": 3,
                "url": "https://github.com/acme/api/milestone/7",
                "updated_at": "2026-11-10T09:00:00Z"
            }]
        })
    );
    assert_eq!(
        asked(&transport),
        vec!["repos/acme/api/milestones?state=open&per_page=100&page=1".to_owned()]
    );
}

#[test]
fn a_named_repo_is_read_and_all_asks_for_the_closed_ones_too() {
    let dir = project(&[
        ("api", "git@github.com:acme/api.git"),
        ("web", "https://github.com/acme/web.git"),
    ]);
    let transport = answering(Ok(listed()));
    let data = ask(
        dir.path(),
        json!({ "repo": "web", "state": "all" }),
        &transport,
    )
    .unwrap();
    assert_eq!(data["path"], "acme/web");
    assert_eq!(data["state"], "all");
    assert_eq!(
        asked(&transport),
        vec!["repos/acme/web/milestones?state=all&per_page=100&page=1".to_owned()]
    );
}

/// Asks that are refused before the forge is asked, with the sentence each answers.
#[test]
fn a_repo_the_workspace_does_not_hold_is_refused_and_the_forge_never_asked() {
    let dir = project(&[
        ("api", "https://github.com/acme/api.git"),
        ("web", "https://github.com/acme/web.git"),
        ("lab", "https://gitlab.com/acme/lab.git"),
        ("notes", ""),
    ]);
    let transport = answering(Ok(listed()));
    for (args, said) in [
        (
            json!({ "repo": "other/repo" }),
            "other/repo is not a repo of this workspace. Name one of api, lab, notes, web.",
        ),
        (
            json!({}),
            "This workspace has 4 repos. Name the one to show with repo: api, lab, notes, web.",
        ),
        (
            json!({ "repo": "lab" }),
            "lab is on GitLab, and purlis reads milestones from GitHub only.",
        ),
        (
            json!({ "repo": "notes" }),
            "notes has no origin on a GitHub host this project declares, so it has no \
             milestones purlis can read.",
        ),
        (
            json!({ "repo": "api", "state": "closed" }),
            "purlis does not list milestones by the state closed: it is open or all.",
        ),
    ] {
        let refused = ask(dir.path(), args.clone(), &transport).unwrap_err();
        assert_eq!(
            refused,
            Unanswered {
                kind: ErrorKind::Refused,
                message: said.into()
            },
            "{args}"
        );
    }
    assert_eq!(asked(&transport), Vec::<String>::new());
}

#[test]
fn a_workspace_with_no_repo_says_so() {
    let dir = project(&[]);
    let refused = ask(dir.path(), json!({}), &answering(Ok(listed()))).unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Refused);
    assert_eq!(
        refused.message,
        "This workspace has no repo yet, so it has no milestones to show."
    );
}

fn refusal(status: u16, headers: &[(&str, &str)]) -> Reply {
    Reply {
        code: 1,
        out: String::new(),
        err: "a token-free refusal".into(),
        status: Some(status),
        headers: headers
            .iter()
            .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
            .collect(),
    }
}

#[test]
fn no_login_no_network_and_a_spent_limit_each_fail_with_a_sentence() {
    let dir = project(&[("api", "https://github.com/acme/api.git")]);
    for (reply, said) in [
        (
            Ok(refusal(401, &[])),
            "purlis has no GitHub login to read the milestones of acme/api. Sign in to GitHub \
             in purlis, or run `gh auth login`.",
        ),
        (
            Err(NoAnswer::Missing("no network".into())),
            "purlis could not reach GitHub for the milestones of acme/api. It asks again in a \
             minute.",
        ),
        (
            Err(NoAnswer::Timeout("the deadline passed".into())),
            "purlis could not reach GitHub for the milestones of acme/api. It asks again in a \
             minute.",
        ),
        (
            Err(NoAnswer::HeldBack {
                said: "spent".into(),
                reset: None,
            }),
            "GitHub's request limit for this hour is spent, so purlis waits to read the \
             milestones of acme/api.",
        ),
        (
            Ok(refusal(404, &[])),
            "GitHub does not show acme/api to this login, so purlis cannot read its milestones.",
        ),
        (
            Err(NoAnswer::Refused("no credential for a chat".into())),
            "purlis may not read the milestones of acme/api with this login.",
        ),
    ] {
        let failed = ask(dir.path(), json!({}), &answering(reply)).unwrap_err();
        assert_eq!(
            failed,
            Unanswered {
                kind: ErrorKind::Failed,
                message: said.into()
            }
        );
    }
}

#[test]
fn the_answer_is_of_the_shape_its_result_describes() {
    let schema = result();
    assert_eq!(schema["type"], "object");
    let item = &schema["properties"]["milestones"]["items"]["properties"];
    for key in [
        "number",
        "title",
        "description",
        "due_on",
        "state",
        "open_issues",
        "closed_issues",
        "url",
        "updated_at",
    ] {
        assert!(item.get(key).is_some(), "{key}");
    }
}
