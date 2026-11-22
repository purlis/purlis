// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The held-act adapters, each against a fake forge: no network. The fake records every call so a
//! test reads exactly what was sent, and which credential sent it.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::*;
use crate::forge::transport::{Endpoint, NoAnswer};

/// Every call a fake saw: who sent it, the method, the path and the call.
type Seen = Arc<Mutex<Vec<(String, String, String, Call)>>>;

type Handler = dyn Fn(&str, &str, &Call) -> Option<(u16, Value)> + Send + Sync;

/// A forge that answers from a function of (method, path) and records what it was sent.
struct Fake {
    who: &'static str,
    answer: Box<Handler>,
    seen: Seen,
}

impl Transport for Fake {
    fn send(&self, _: &Forge, call: &Call) -> Result<Reply, NoAnswer> {
        let (method, path) = match &call.endpoint {
            Endpoint::Rest { method, path } => (method.map_or("GET", Method::word), path.clone()),
            Endpoint::Graphql => ("POST", "graphql".to_owned()),
        };
        self.seen.lock().unwrap().push((
            self.who.to_owned(),
            method.to_owned(),
            path.clone(),
            call.clone(),
        ));
        let (status, body) =
            (self.answer)(method, &path, call).unwrap_or((404, json!({"message": "Not Found"})));
        let ok = status < 400;
        Ok(Reply {
            code: i32::from(!ok),
            out: if ok { body.to_string() } else { String::new() },
            err: if ok {
                String::new()
            } else {
                format!("{body} (HTTP {status})")
            },
            status: Some(status),
            headers: Vec::new(),
        })
    }

    fn check_auth(&self, _: &Forge) -> Result<(), ForgeError> {
        Ok(())
    }
}

struct Files(Vec<TipChange>);

impl TipFiles for Files {
    fn changes(&self, _: &HeadSha, _: &HeadSha) -> Result<Vec<TipChange>, String> {
        Ok(self.0.clone())
    }
    fn message(&self, _: &HeadSha) -> Result<String, String> {
        Ok("Pin the runner".to_owned())
    }
}

struct Second {
    answer: Arc<Handler>,
    seen: Seen,
    used: Arc<Mutex<Vec<String>>>,
}

impl WorkflowTransports for Second {
    fn with(&self, _: &Forge, secret: &Secret) -> Result<Arc<dyn Transport>, String> {
        self.used.lock().unwrap().push(secret.expose().to_owned());
        let answer = self.answer.clone();
        Ok(Arc::new(Fake {
            who: "workflow",
            answer: Box::new(move |m, p, c| answer(m, p, c)),
            seen: self.seen.clone(),
        }))
    }
}

fn sha(c: char) -> HeadSha {
    HeadSha::try_from(c.to_string().repeat(40)).unwrap()
}
fn repo(path: &str) -> Repo {
    Repo::try_from(path).unwrap()
}
fn tag(name: &str) -> TagName {
    TagName::try_from(name.to_owned()).unwrap()
}
fn field(call: &Call, name: &str) -> Option<String> {
    call.fields.iter().find_map(|f| match f {
        Field::Text(n, v) | Field::Typed(n, v) if n == name => Some(v.clone()),
        _ => None,
    })
}
fn all(call: &Call, name: &str) -> Vec<String> {
    call.fields
        .iter()
        .filter_map(|f| match f {
            Field::Text(n, v) | Field::Typed(n, v) if n == name => Some(v.clone()),
            _ => None,
        })
        .collect()
}

fn build(
    kind: Kind,
    answer: impl Fn(&str, &str, &Call) -> Option<(u16, Value)> + Send + Sync + 'static,
    files: Vec<TipChange>,
) -> (Box<dyn HeldForge>, Seen, Arc<Mutex<Vec<String>>>) {
    let answer: Arc<Handler> = Arc::new(answer);
    let seen: Seen = Arc::default();
    let used = Arc::new(Mutex::new(Vec::new()));
    let person = Fake {
        who: "person",
        answer: Box::new({
            let answer = answer.clone();
            move |m, p, c| answer(m, p, c)
        }),
        seen: seen.clone(),
    };
    let forge = Forge {
        kind,
        host: match kind {
            Kind::GitHub => "github.com".to_owned(),
            Kind::GitLab => "gitlab.com".to_owned(),
        },
    };
    let held = held_forge(
        forge,
        Arc::new(person),
        Arc::new(Files(files)),
        Arc::new(Second {
            answer,
            seen: seen.clone(),
            used: used.clone(),
        }),
    );
    (held, seen, used)
}

fn calls(seen: &Seen) -> Vec<(String, String, String)> {
    seen.lock()
        .unwrap()
        .iter()
        .map(|(who, m, p, _)| (who.clone(), m.clone(), p.clone()))
        .collect()
}

fn workflow_file() -> Vec<TipChange> {
    vec![
        TipChange {
            path: ".github/workflows/ci.yml".into(),
            content: Some("on: push\n".into()),
            executable: false,
            added: false,
        },
        TipChange {
            path: "old.txt".into(),
            content: None,
            executable: false,
            added: false,
        },
    ]
}

// GitHub ----------------------------------------------------------------------------------

fn github_reads(method: &str, path: &str, _: &Call) -> Option<(u16, Value)> {
    match (method, path) {
        ("GET", "repos/acme/app") => Some((
            200,
            json!({"default_branch": "main", "id": 4242, "allow_forking": true}),
        )),
        ("GET", "repos/acme/app/git/ref/heads/main") => Some((
            200,
            json!({"object": {"type": "commit", "sha": sha('1').as_str()}}),
        )),
        ("GET", "repos/acme/app/git/ref/tags/v1") => Some((
            200,
            json!({"object": {"type": "tag", "sha": sha('7').as_str()}}),
        )),
        ("GET", p) if p == format!("repos/acme/app/git/tags/{}", sha('7').as_str()) => Some((
            200,
            json!({"object": {"type": "commit", "sha": sha('3').as_str()}}),
        )),
        ("GET", "repos/acme/app/commits/feature") => Some((200, json!({"sha": sha('2').as_str()}))),
        ("GET", p) if p.starts_with("repos/acme/app/compare/") => {
            let ahead = p.contains(sha('1').as_str());
            Some((
                200,
                json!({"status": if ahead { "ahead" } else { "diverged" }}),
            ))
        }
        ("GET", "user") => Some((200, json!({"login": "ada"}))),
        _ => None,
    }
}

#[test]
fn github_reads_say_what_an_ask_shows() {
    let (forge, _, _) = build(Kind::GitHub, github_reads, vec![]);
    let app = repo("acme/app");
    assert_eq!(forge.rev_commit(&app, "feature").unwrap(), sha('2'));
    let (base, tip) = forge.default_branch(&app).unwrap();
    assert_eq!((base.as_str(), tip), ("main", sha('1')));
    assert_eq!(
        forge
            .branch_tip(&app, &Base::try_from("nope").unwrap())
            .unwrap(),
        None
    );
    assert!(
        forge
            .on_branch(&app, &Base::try_from("main").unwrap(), &sha('1'))
            .unwrap()
    );
    assert!(
        !forge
            .on_branch(&app, &Base::try_from("main").unwrap(), &sha('2'))
            .unwrap()
    );
    assert_eq!(forge.source_id(&app).unwrap(), "4242");
    assert_eq!(forge.account().unwrap(), "ada");
    assert!(forge.may_fork(&app).unwrap());
    assert_eq!(forge.ci_config_path(&app).unwrap(), None);
}

#[test]
fn github_follows_an_annotated_tag_to_its_commit_and_finds_none_for_a_missing_one() {
    let (forge, _, _) = build(Kind::GitHub, github_reads, vec![]);
    let app = repo("acme/app");
    assert_eq!(forge.tag_at(&app, &tag("v1")).unwrap(), Some(sha('3')));
    assert_eq!(forge.tag_at(&app, &tag("v2")).unwrap(), None);
}

#[test]
fn github_makes_a_tag_as_a_ref_at_the_commit_and_nothing_else() {
    let (forge, seen, _) = build(Kind::GitHub, |_, _, _| Some((201, json!({}))), vec![]);
    forge
        .create_tag(&repo("acme/app"), &tag("v1"), &sha('2'))
        .unwrap();
    let log = seen.lock().unwrap();
    assert_eq!(log.len(), 1);
    let (who, method, path, call) = &log[0];
    assert_eq!(
        (who.as_str(), method.as_str(), path.as_str()),
        ("person", "POST", "repos/acme/app/git/refs")
    );
    assert_eq!(field(call, "ref").as_deref(), Some("refs/tags/v1"));
    assert_eq!(field(call, "sha").as_deref(), Some(sha('2').as_str()));
}

#[test]
fn github_never_moves_a_tag_that_exists_the_forge_refuses_and_the_words_come_back() {
    let (forge, _, _) = build(
        Kind::GitHub,
        |_, _, _| Some((422, json!({"message": "Reference already exists"}))),
        vec![],
    );
    let said = forge
        .create_tag(&repo("acme/app"), &tag("v1"), &sha('2'))
        .unwrap_err();
    assert!(said.contains("Reference already exists"), "{said}");
}

#[test]
fn github_makes_the_release_against_the_tag_with_no_target_commitish() {
    let (forge, seen, _) = build(Kind::GitHub, |_, _, _| Some((201, json!({}))), vec![]);
    forge
        .create_release(&repo("acme/app"), &tag("v1"), "One", "Notes", false, true)
        .unwrap();
    let log = seen.lock().unwrap();
    let (_, method, path, call) = &log[0];
    assert_eq!(
        (method.as_str(), path.as_str()),
        ("POST", "repos/acme/app/releases")
    );
    assert_eq!(field(call, "tag_name").as_deref(), Some("v1"));
    assert_eq!(field(call, "body").as_deref(), Some("Notes"));
    assert_eq!(field(call, "prerelease").as_deref(), Some("true"));
    assert_eq!(field(call, "target_commitish"), None);
}

#[test]
fn github_forks_into_the_persons_account_with_no_organization_named() {
    let (forge, seen, _) = build(
        Kind::GitHub,
        |method, _, _| match method {
            "GET" => Some((200, json!({"id": 4242}))),
            _ => Some((202, json!({"full_name": "ada/app", "id": 77}))),
        },
        vec![],
    );
    let made = forge.make_fork(&repo("acme/app"), "4242").unwrap();
    assert_eq!((made.path.as_str(), made.id.as_str()), ("ada/app", "77"));
    let log = seen.lock().unwrap();
    let fork = log.last().unwrap();
    assert!(fork.3.fields.is_empty(), "{:?}", fork.3.fields);
    assert_eq!(fork.2, "repos/acme/app/forks");
}

fn github_push_forge(branch_exists: bool) -> (Box<dyn HeldForge>, Seen, Arc<Mutex<Vec<String>>>) {
    build(
        Kind::GitHub,
        move |method, path, _| match (method, path) {
            ("GET", "repos/acme/app") => Some((200, json!({"default_branch": "main"}))),
            ("GET", "repos/acme/app/git/ref/heads/ci") if branch_exists => Some((
                200,
                json!({"object": {"type": "commit", "sha": sha('1').as_str()}}),
            )),
            ("GET", "repos/acme/app/git/ref/heads/main") => Some((
                200,
                json!({"object": {"type": "commit", "sha": sha('1').as_str()}}),
            )),
            ("GET", p) if p == format!("repos/acme/app/git/commits/{}", sha('1').as_str()) => {
                Some((200, json!({"tree": {"sha": "t0"}})))
            }
            ("POST", "repos/acme/app/git/blobs") => Some((201, json!({"sha": "b1"}))),
            ("POST", "repos/acme/app/git/trees") => Some((201, json!({"sha": "t1"}))),
            ("POST", "repos/acme/app/git/commits") => Some((201, json!({"sha": "c1"}))),
            ("PATCH", p) if branch_exists && p == "repos/acme/app/git/refs/heads/ci" => {
                Some((200, json!({})))
            }
            ("POST", "repos/acme/app/git/refs") if !branch_exists => Some((201, json!({}))),
            _ => None,
        },
        workflow_file(),
    )
}

#[test]
fn github_pushes_one_commit_on_the_base_with_the_second_credential_and_a_lease() {
    let (forge, seen, used) = github_push_forge(true);
    let secret = Secret::new("wf-token".to_owned());
    forge
        .push_tip(
            &repo("acme/app"),
            &Base::try_from("ci").unwrap(),
            &sha('9'),
            &sha('1'),
            false,
            Some(&secret),
        )
        .unwrap();
    assert_eq!(used.lock().unwrap().as_slice(), ["wf-token"]);
    let log = seen.lock().unwrap();
    // Every call of the push is the second credential's, and none is the everyday sign-in's.
    assert!(
        log.iter().all(|(who, ..)| who == "workflow"),
        "{:?}",
        calls(&seen)
    );
    let tree = log
        .iter()
        .find(|(_, _, p, _)| p.ends_with("git/trees"))
        .unwrap();
    assert_eq!(field(&tree.3, "base_tree").as_deref(), Some("t0"));
    assert_eq!(
        all(&tree.3, "tree[][path]"),
        [".github/workflows/ci.yml", "old.txt"]
    );
    assert_eq!(all(&tree.3, "tree[][sha]"), ["b1", "null"]);
    let commit = log
        .iter()
        .find(|(_, _, p, _)| p.ends_with("git/commits") && p.starts_with("repos"))
        .unwrap();
    assert_eq!(all(&commit.3, "parents[]"), [sha('1').as_str()]);
    assert_eq!(
        field(&commit.3, "message").as_deref(),
        Some("Pin the runner")
    );
    let moved = log.last().unwrap();
    assert_eq!(
        (moved.1.as_str(), moved.2.as_str()),
        ("PATCH", "repos/acme/app/git/refs/heads/ci")
    );
    assert_eq!(field(&moved.3, "force").as_deref(), Some("false"));
}

#[test]
fn github_makes_a_branch_that_was_not_there_by_creating_the_ref_not_moving_one() {
    let (forge, seen, _) = github_push_forge(false);
    let secret = Secret::new("wf".to_owned());
    forge
        .push_tip(
            &repo("acme/app"),
            &Base::try_from("ci").unwrap(),
            &sha('9'),
            &sha('1'),
            true,
            Some(&secret),
        )
        .unwrap();
    let log = seen.lock().unwrap();
    let last = log.last().unwrap();
    assert_eq!(
        (last.1.as_str(), last.2.as_str()),
        ("POST", "repos/acme/app/git/refs")
    );
    assert_eq!(field(&last.3, "ref").as_deref(), Some("refs/heads/ci"));
    // The base is the default branch's tip, read at the push.
    let commit = log
        .iter()
        .find(|(_, _, p, _)| p.ends_with("git/commits") && p.starts_with("repos"))
        .unwrap();
    assert_eq!(all(&commit.3, "parents[]"), [sha('1').as_str()]);
}

#[test]
fn github_refuses_a_branch_that_moved_and_says_the_forges_words() {
    let (forge, _, _) = build(
        Kind::GitHub,
        |method, path, _| match (method, path) {
            ("GET", "repos/acme/app/git/ref/heads/ci") => Some((
                200,
                json!({"object": {"type": "commit", "sha": sha('1').as_str()}}),
            )),
            ("GET", p) if p.contains("git/commits/") => Some((200, json!({"tree": {"sha": "t0"}}))),
            ("POST", "repos/acme/app/git/blobs") => Some((201, json!({"sha": "b1"}))),
            ("POST", "repos/acme/app/git/trees") => Some((201, json!({"sha": "t1"}))),
            ("POST", "repos/acme/app/git/commits") => Some((201, json!({"sha": "c1"}))),
            ("PATCH", _) => Some((422, json!({"message": "Update is not a fast forward"}))),
            _ => None,
        },
        workflow_file(),
    );
    let secret = Secret::new("wf".to_owned());
    let said = forge
        .push_tip(
            &repo("acme/app"),
            &Base::try_from("ci").unwrap(),
            &sha('9'),
            &sha('1'),
            false,
            Some(&secret),
        )
        .unwrap_err();
    assert!(said.contains("not a fast forward"), "{said}");
}

#[test]
fn github_pushes_nothing_without_the_second_credential() {
    let (forge, seen, _) = github_push_forge(true);
    let said = forge
        .push_tip(
            &repo("acme/app"),
            &Base::try_from("ci").unwrap(),
            &sha('9'),
            &sha('1'),
            false,
            None,
        )
        .unwrap_err();
    assert!(said.contains("second sign-in"), "{said}");
    assert!(seen.lock().unwrap().is_empty());
}

// GitLab ----------------------------------------------------------------------------------

fn gitlab_reads(method: &str, path: &str, _: &Call) -> Option<(u16, Value)> {
    match (method, path) {
        ("GET", "projects/acme%2Fapp") => Some((
            200,
            json!({"default_branch": "main", "id": 9, "ci_config_path": "ci/pipeline.yml",
                   "forking_access_level": "enabled"}),
        )),
        ("GET", "projects/acme%2Fapp/repository/branches/main") => {
            Some((200, json!({"commit": {"id": sha('1').as_str()}})))
        }
        ("GET", "projects/acme%2Fapp/repository/tags/v1") => {
            Some((200, json!({"commit": {"id": sha('3').as_str()}})))
        }
        ("GET", p) if p.starts_with("projects/acme%2Fapp/repository/merge_base") => {
            // The merge base of a commit with main is the commit only where main has it.
            let id = if p.contains(sha('1').as_str()) {
                sha('1')
            } else {
                sha('0')
            };
            Some((200, json!({"id": id.as_str()})))
        }
        ("GET", "projects/acme%2Fapp/repository/commits/feature") => {
            Some((200, json!({"id": sha('2').as_str()})))
        }
        ("GET", "user") => Some((200, json!({"username": "ada"}))),
        _ => None,
    }
}

#[test]
fn gitlab_reads_say_what_an_ask_shows() {
    let (forge, _, _) = build(Kind::GitLab, gitlab_reads, vec![]);
    let app = repo("acme/app");
    assert_eq!(forge.rev_commit(&app, "feature").unwrap(), sha('2'));
    let (base, tip) = forge.default_branch(&app).unwrap();
    assert_eq!((base.as_str(), tip), ("main", sha('1')));
    assert_eq!(forge.tag_at(&app, &tag("v1")).unwrap(), Some(sha('3')));
    assert_eq!(forge.tag_at(&app, &tag("v2")).unwrap(), None);
    assert!(
        forge
            .on_branch(&app, &Base::try_from("main").unwrap(), &sha('1'))
            .unwrap()
    );
    assert!(
        !forge
            .on_branch(&app, &Base::try_from("main").unwrap(), &sha('2'))
            .unwrap()
    );
    assert_eq!(
        forge.ci_config_path(&app).unwrap().as_deref(),
        Some("ci/pipeline.yml")
    );
    assert_eq!(forge.source_id(&app).unwrap(), "9");
    assert_eq!(forge.account().unwrap(), "ada");
    assert!(forge.may_fork(&app).unwrap());
}

#[test]
fn gitlab_makes_a_tag_at_the_commit_and_a_release_with_no_ref() {
    let (forge, seen, _) = build(Kind::GitLab, |_, _, _| Some((201, json!({}))), vec![]);
    let app = repo("acme/app");
    forge.create_tag(&app, &tag("v1"), &sha('2')).unwrap();
    forge
        .create_release(&app, &tag("v1"), "One", "Notes", false, false)
        .unwrap();
    let log = seen.lock().unwrap();
    assert_eq!(log[0].2, "projects/acme%2Fapp/repository/tags");
    assert_eq!(field(&log[0].3, "ref").as_deref(), Some(sha('2').as_str()));
    assert_eq!(log[1].2, "projects/acme%2Fapp/releases");
    assert_eq!(field(&log[1].3, "tag_name").as_deref(), Some("v1"));
    assert_eq!(field(&log[1].3, "description").as_deref(), Some("Notes"));
    assert_eq!(field(&log[1].3, "ref"), None);
}

#[test]
fn gitlab_has_no_draft_or_prerelease_so_none_is_made_as_something_else() {
    let (forge, seen, _) = build(Kind::GitLab, |_, _, _| Some((201, json!({}))), vec![]);
    let said = forge
        .create_release(&repo("acme/app"), &tag("v1"), "", "n", true, false)
        .unwrap_err();
    assert!(said.contains("no draft"), "{said}");
    assert!(seen.lock().unwrap().is_empty());
}

#[test]
fn gitlab_forks_with_no_namespace_named() {
    let (forge, seen, _) = build(
        Kind::GitLab,
        |_, _, _| Some((201, json!({"path_with_namespace": "ada/app", "id": 12}))),
        vec![],
    );
    let made = forge.make_fork(&repo("acme/app"), "9").unwrap();
    assert_eq!(made.path, "ada/app");
    assert!(seen.lock().unwrap()[0].3.fields.is_empty());
}

fn gitlab_push_forge(parent_of_commit: char) -> (Box<dyn HeldForge>, Seen) {
    let (forge, seen, _) = build(
        Kind::GitLab,
        move |method, path, _| match (method, path) {
            ("GET", "projects/acme%2Fapp/repository/branches/ci") => {
                Some((200, json!({"commit": {"id": sha('1').as_str()}})))
            }
            ("POST", "projects/acme%2Fapp/repository/commits") => Some((
                201,
                json!({"id": "new", "parent_ids": [sha(parent_of_commit).as_str()]}),
            )),
            _ => None,
        },
        vec![
            TipChange {
                path: ".gitlab-ci.yml".into(),
                content: Some("a: 1\n".into()),
                executable: false,
                added: true,
            },
            TipChange {
                path: "x.sh".into(),
                content: Some("#!/bin/sh\n".into()),
                executable: true,
                added: false,
            },
            TipChange {
                path: "gone".into(),
                content: None,
                executable: false,
                added: false,
            },
        ],
    );
    (forge, seen)
}

#[test]
fn gitlab_pushes_the_changes_as_one_commit_with_the_everyday_sign_in() {
    let (forge, seen) = gitlab_push_forge('1');
    forge
        .push_tip(
            &repo("acme/app"),
            &Base::try_from("ci").unwrap(),
            &sha('9'),
            &sha('1'),
            false,
            None,
        )
        .unwrap();
    let log = seen.lock().unwrap();
    assert!(log.iter().all(|(who, ..)| who == "person"));
    let commit = log.last().unwrap();
    assert_eq!(commit.2, "projects/acme%2Fapp/repository/commits");
    assert_eq!(field(&commit.3, "branch").as_deref(), Some("ci"));
    assert_eq!(
        all(&commit.3, "actions[][action]"),
        ["create", "update", "delete"]
    );
    assert_eq!(
        all(&commit.3, "actions[][file_path]"),
        [".gitlab-ci.yml", "x.sh", "gone"]
    );
    assert_eq!(field(&commit.3, "start_sha"), None);
}

#[test]
fn gitlab_pushes_nothing_when_the_branch_is_no_longer_at_the_base() {
    let (forge, seen) = gitlab_push_forge('1');
    let said = forge
        .push_tip(
            &repo("acme/app"),
            &Base::try_from("ci").unwrap(),
            &sha('9'),
            &sha('5'),
            false,
            None,
        )
        .unwrap_err();
    assert!(said.contains("no longer at"), "{said}");
    assert!(seen.lock().unwrap().iter().all(|(_, m, ..)| m == "GET"));
}

#[test]
fn gitlab_says_so_when_the_branch_moved_between_the_read_and_the_commit() {
    let (forge, _) = gitlab_push_forge('4');
    let said = forge
        .push_tip(
            &repo("acme/app"),
            &Base::try_from("ci").unwrap(),
            &sha('9'),
            &sha('1'),
            false,
            None,
        )
        .unwrap_err();
    assert!(said.contains("moved while the push was made"), "{said}");
}

#[test]
fn a_credential_source_is_the_only_token_the_person_transport_reads() {
    use crate::forge::credential::{Credential, CredentialError, CredentialSource};
    struct None_;
    impl CredentialSource for None_ {
        fn credential_for(&self, host: &str) -> Result<Credential, CredentialError> {
            Err(CredentialError::None {
                host: host.to_owned(),
                how: "Sign in.".to_owned(),
            })
        }
    }
    let tokens = CredentialTokens::new("github.com", Arc::new(None_));
    assert!(tokens.token().is_err());
}

// Round 2 ---------------------------------------------------------------------------------

const HOSTILE_REVS: [&str; 5] = [
    "../../../other/private/commits/main",
    "/etc/passwd",
    "main@{1}",
    "a b",
    "x\u{0}y",
];

#[test]
fn a_rev_that_walks_the_api_is_refused_before_any_call_on_both_adapters() {
    for kind in [Kind::GitHub, Kind::GitLab] {
        let (forge, seen, _) = build(kind, |_, _, _| Some((200, json!({}))), vec![]);
        for rev in HOSTILE_REVS {
            assert!(forge.rev_commit(&repo("acme/app"), rev).is_err(), "{rev}");
        }
        assert!(seen.lock().unwrap().is_empty(), "{kind:?}");
    }
}

#[test]
fn a_branch_name_with_a_slash_is_one_path_segment_of_the_commit_route() {
    let (forge, seen, _) = build(
        Kind::GitHub,
        |_, _, _| Some((200, json!({"sha": sha('2').as_str()}))),
        vec![],
    );
    forge.rev_commit(&repo("acme/app"), "feature/x").unwrap();
    assert_eq!(
        seen.lock().unwrap()[0].2,
        "repos/acme/app/commits/feature%2Fx"
    );
}

#[test]
fn github_parents_a_new_branch_on_the_shown_base_though_the_default_branch_moved() {
    let (forge, seen, _) = build(
        Kind::GitHub,
        |method, path, _| match (method, path) {
            // The default branch is now at 5, not the 1 the ask showed.
            ("GET", "repos/acme/app/git/ref/heads/main") => Some((
                200,
                json!({"object": {"type": "commit", "sha": sha('5').as_str()}}),
            )),
            ("GET", p) if p == format!("repos/acme/app/git/commits/{}", sha('1').as_str()) => {
                Some((200, json!({"tree": {"sha": "t0"}})))
            }
            ("POST", "repos/acme/app/git/blobs") => Some((201, json!({"sha": "b1"}))),
            ("POST", "repos/acme/app/git/trees") => Some((201, json!({"sha": "t1"}))),
            ("POST", "repos/acme/app/git/commits") => Some((201, json!({"sha": "c1"}))),
            ("POST", "repos/acme/app/git/refs") => Some((201, json!({}))),
            _ => None,
        },
        workflow_file(),
    );
    let secret = Secret::new("wf".to_owned());
    forge
        .push_tip(
            &repo("acme/app"),
            &Base::try_from("ci").unwrap(),
            &sha('9'),
            &sha('1'),
            true,
            Some(&secret),
        )
        .unwrap();
    let log = seen.lock().unwrap();
    let commit = log
        .iter()
        .find(|(_, m, p, _)| m == "POST" && p == "repos/acme/app/git/commits")
        .unwrap();
    assert_eq!(all(&commit.3, "parents[]"), [sha('1').as_str()]);
    assert!(log.iter().all(|(_, _, p, _)| !p.ends_with("heads/main")));
}

#[test]
fn github_refuses_a_branch_rewound_to_an_ancestor_of_the_base() {
    let (forge, seen, _) = build(
        Kind::GitHub,
        |method, path, _| match (method, path) {
            ("GET", "repos/acme/app/git/ref/heads/ci") => Some((
                200,
                json!({"object": {"type": "commit", "sha": sha('0').as_str()}}),
            )),
            _ => Some((200, json!({"sha": "x", "tree": {"sha": "t"}}))),
        },
        workflow_file(),
    );
    let secret = Secret::new("wf".to_owned());
    let said = forge
        .push_tip(
            &repo("acme/app"),
            &Base::try_from("ci").unwrap(),
            &sha('9'),
            &sha('1'),
            false,
            Some(&secret),
        )
        .unwrap_err();
    assert!(said.contains("no longer at"), "{said}");
    assert!(seen.lock().unwrap().iter().all(|(_, m, ..)| m == "GET"));
}

#[test]
fn gitlab_starts_a_new_branch_at_the_shown_base_though_the_default_branch_moved() {
    let (forge, seen, _) = build(
        Kind::GitLab,
        |method, path, _| match (method, path) {
            ("GET", "projects/acme%2Fapp") => Some((200, json!({"default_branch": "main"}))),
            ("GET", "projects/acme%2Fapp/repository/branches/main") => {
                Some((200, json!({"commit": {"id": sha('5').as_str()}})))
            }
            ("POST", "projects/acme%2Fapp/repository/commits") => {
                Some((201, json!({"id": "new", "parent_ids": [sha('1').as_str()]})))
            }
            _ => None,
        },
        vec![TipChange {
            path: ".gitlab-ci.yml".into(),
            content: Some("a: 1\n".into()),
            executable: false,
            added: true,
        }],
    );
    forge
        .push_tip(
            &repo("acme/app"),
            &Base::try_from("ci").unwrap(),
            &sha('9'),
            &sha('1'),
            true,
            None,
        )
        .unwrap();
    let log = seen.lock().unwrap();
    let commit = log.last().unwrap();
    assert_eq!(
        field(&commit.3, "start_sha").as_deref(),
        Some(sha('1').as_str())
    );
}

#[test]
fn gitlab_forks_by_the_numeric_id_the_ask_showed() {
    let (forge, seen, _) = build(
        Kind::GitLab,
        |_, _, _| Some((201, json!({"path_with_namespace": "ada/app", "id": 12}))),
        vec![],
    );
    forge.make_fork(&repo("acme/app"), "9").unwrap();
    assert_eq!(seen.lock().unwrap()[0].2, "projects/9/fork");
}

#[test]
fn github_forks_nothing_when_the_repo_is_not_the_one_the_ask_showed() {
    let (forge, seen, _) = build(
        Kind::GitHub,
        |_, _, _| Some((200, json!({"id": 1}))),
        vec![],
    );
    assert!(forge.make_fork(&repo("acme/app"), "4242").is_err());
    assert!(seen.lock().unwrap().iter().all(|(_, m, ..)| m == "GET"));
}
