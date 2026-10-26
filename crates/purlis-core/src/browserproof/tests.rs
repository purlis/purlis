// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use base64::Engine as _;
use purlis_session_protocol::browsing::{Act, Browsed, PageSaid};
use serde_json::{Value, json};

use super::*;
use crate::browsertimeline::{Seen, acted, record};
use crate::forge::recorded::Recorded;

const CHAT: &str = "01JB0000000000000000000000";
const PR: &str = "https://github.com/acme/api/pull/12";

fn project() -> tempfile::TempDir {
    tempfile::tempdir().expect("a project")
}

/// A screenshot of `bytes` at `at`, taken on a page whose address and the reference the agent
/// clicked hold what the agent wrote.
fn took(root: &Path, at: u64, bytes: &[u8]) {
    let browsed = Browsed {
        page: Some(PageSaid {
            page: "p1".into(),
            address: "https://staging.example.test/agent-typed-path".into(),
            title: "Cart".into(),
            current: true,
        }),
        settled: true,
        png: Some(bytes.to_vec()),
        ..Browsed::default()
    };
    let happened = acted(
        "b1",
        &Act::Screenshot { page: None },
        &browsed,
        at,
        Seen::default(),
    )
    .expect("a moment");
    record(root, CHAT, happened).expect("recorded");
}

fn shots(root: &Path) -> Vec<u64> {
    browsertimeline::read(root, CHAT)
        .iter()
        .filter_map(|moment| moment.shot)
        .collect()
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn get(path: &str, out: Value) -> Value {
    json!({"call": {"endpoint": {"rest": {"path": path}}, "fields": []},
           "reply": {"code": 0, "out": out.to_string(), "err": ""}})
}

fn write(method: &str, path: &str, fields: Value, out: Value) -> Value {
    json!({"call": {"endpoint": {"rest": {"method": method, "path": path}}, "fields": fields},
           "reply": {"code": 0, "out": out.to_string(), "err": ""}})
}

/// A GitHub whose request 12 is open and whose proof branch is not there yet, answering the
/// keeping of `pngs` and a comment of `body`.
fn github(pngs: &[&[u8]], body: &str) -> Arc<Recorded> {
    let commit = "6".repeat(40);
    let mut exchanges = vec![get(
        "repos/acme/api/pulls/12",
        json!({"number": 12, "state": "open", "merged": false}),
    )];
    let mut entries = Vec::new();
    for (n, png) in pngs.iter().enumerate() {
        let blob = format!("{n}").repeat(40);
        exchanges.push(write(
            "POST",
            "repos/acme/api/git/blobs",
            json!([{"text": ["content", b64(png)]}, {"text": ["encoding", "base64"]}]),
            json!({"sha": blob}),
        ));
        entries.extend([
            json!({"text": ["tree[][path]", format!("browser-proof-{}.png", n + 1)]}),
            json!({"text": ["tree[][mode]", "100644"]}),
            json!({"text": ["tree[][type]", "blob"]}),
            json!({"text": ["tree[][sha]", blob]}),
        ]);
    }
    let branch = "purlis/browser-proof/pr-12";
    exchanges.push(json!({
        "call": {"endpoint": {"rest": {"path": format!("repos/acme/api/git/ref/heads/{branch}")}},
                 "fields": []},
        "reply": {"code": 1, "out": "{\"message\":\"Not Found\"}", "err": "HTTP 404",
                  "status": 404}
    }));
    exchanges.push(write(
        "POST",
        "repos/acme/api/git/trees",
        Value::Array(entries),
        json!({"sha": "5".repeat(40)}),
    ));
    exchanges.push(write(
        "POST",
        "repos/acme/api/git/commits",
        json!([{"text": ["message", "Browser proof for pull request #12"]},
               {"text": ["tree", "5".repeat(40)]}]),
        json!({"sha": commit}),
    ));
    exchanges.push(write(
        "POST",
        "repos/acme/api/git/refs",
        json!([{"text": ["ref", format!("refs/heads/{branch}")]}, {"text": ["sha", commit]}]),
        json!({"ref": format!("refs/heads/{branch}")}),
    ));
    exchanges.push(write(
        "POST",
        "repos/acme/api/issues/12/comments",
        json!([{"text": ["body", body]}]),
        json!({"html_url": "https://github.com/acme/api/pull/12#issuecomment-1"}),
    ));
    Arc::new(Recorded::parse(&json!({"source": "GitHub REST API version 2022-11-28, as the forge contract recordings", "exchanges": exchanges}).to_string()).expect("a recording"))
}

fn over(recorded: &Arc<Recorded>) -> impl Fn(&Forge) -> Result<Box<dyn ForgeBackend>, String> {
    let recorded = recorded.clone();
    move |forge: &Forge| Ok(forge.backend_over(recorded.clone()))
}

fn never(_: &Forge) -> Result<Box<dyn ForgeBackend>, String> {
    panic!("the forge was asked")
}

#[test]
fn only_the_screenshots_the_person_picked_are_shown_with_where_they_go() {
    let dir = project();
    took(dir.path(), 1, b"\x89PNG one");
    took(dir.path(), 2, b"\x89PNG two");
    let all = shots(dir.path());

    let shown = show(dir.path(), CHAT, PR, &all[1..]).expect("shown");

    assert_eq!(shown.request, PR);
    assert_eq!(shown.number, "#12");
    assert_eq!(shown.path, "acme/api");
    assert_eq!(
        shown.kept_at,
        "on the branch purlis/browser-proof/pr-12 of acme/api, which whoever can read acme/api \
         can read"
    );
    assert_eq!(
        shown.shots,
        [ShownShot {
            shot: all[1],
            at: 2,
            digest: "4ce1f4445c0247b6b59c2c6abdc3f68d4ecf5f165db2a0aaa9c9edb6f429c94e".into(),
        }]
    );
    assert!(
        shown
            .words
            .starts_with("### Browser proof\n\n1 screenshot of")
    );
}

#[test]
fn nothing_picked_or_a_screenshot_no_longer_kept_is_refused() {
    let dir = project();
    took(dir.path(), 1, b"\x89PNG one");
    assert_eq!(
        show(dir.path(), CHAT, PR, &[]),
        Err("Pick at least one screenshot to attach.".to_string())
    );
    assert_eq!(
        show(dir.path(), CHAT, PR, &[999]),
        Err("A screenshot you picked is no longer kept.".to_string())
    );
    assert_eq!(
        show(
            dir.path(),
            "01JB0000000000000000000001",
            PR,
            &shots(dir.path())
        ),
        Err("A screenshot you picked is no longer kept.".to_string()),
        "another chat's screenshots are not this chat's"
    );
}

#[test]
fn only_a_request_on_a_forge_the_project_knows_is_one_to_attach_to() {
    let dir = project();
    let request = request_of(
        dir.path(),
        "https://gitlab.com/acme/plat/api/-/merge_requests/7",
    )
    .expect("a merge request");
    assert_eq!(
        (request.forge.kind, request.path.as_str(), request.number),
        (Kind::GitLab, "acme/plat/api", 7)
    );
    for refused in [
        "https://evil.example/acme/api/pull/12",
        "http://github.com/acme/api/pull/12",
        "https://github.com/acme/api/issues/12",
        "https://github.com/acme/api/pull/012",
        "https://github.com/acme/api/pull/12?x=1",
        "https://github.com/a/b/c/pull/12",
        "https://github.com/acme/../pull/12",
        "https://github.com/acme/api/-/merge_requests/12",
    ] {
        assert!(request_of(dir.path(), refused).is_err(), "{refused}");
    }
    // An address with user information names another host than the one it seems to.
    let with_user = format!("https://user{}github.com/acme/api/pull/12", '\u{40}');
    assert!(request_of(dir.path(), &with_user).is_err());
}

#[test]
fn attaching_sends_the_shown_bytes_and_purlis_words_and_nothing_the_agent_wrote() {
    let dir = project();
    took(dir.path(), 1, b"\x89PNG one");
    took(dir.path(), 2, b"\x89PNG two");
    let all = shots(dir.path());
    let shown = show(dir.path(), CHAT, PR, &all).expect("shown");
    let body = format!(
        "{}\n\n![Browser screenshot 1](https://github.com/acme/api/raw/{c}/browser-proof-1.png)\
         \n\n![Browser screenshot 2](https://github.com/acme/api/raw/{c}/browser-proof-2.png)",
        words(2),
        c = "6".repeat(40)
    );
    assert!(!body.contains("staging") && !body.contains("agent-typed"));
    let recorded = github(&[b"\x89PNG one", b"\x89PNG two"], &body);

    let attached = attach(
        dir.path(),
        CHAT,
        &shown,
        &over(&recorded),
        &Caller::window(),
    );

    assert_eq!(
        attached,
        Ok(Attached {
            comment: "https://github.com/acme/api/pull/12#issuecomment-1".into()
        })
    );
    assert_eq!(recorded.unspent(), Vec::new(), "every exchange asked");
}

#[test]
fn a_shown_proof_that_changed_since_is_refused_before_the_forge_is_asked() {
    let dir = project();
    took(dir.path(), 1, b"\x89PNG one");
    let shown = show(dir.path(), CHAT, PR, &shots(dir.path())).expect("shown");

    let mut other = shown.clone();
    other.shots[0].digest = "0".repeat(64);
    let mut reworded = shown.clone();
    reworded.words = "Typed by the agent".into();
    for changed in [other, reworded] {
        assert_eq!(
            attach(dir.path(), CHAT, &changed, &never, &Caller::window()),
            Err(super::changed())
        );
    }

    browsertimeline::remove(dir.path(), CHAT).expect("the chat closed");
    assert_eq!(
        attach(dir.path(), CHAT, &shown, &never, &Caller::window()),
        Err(super::changed()),
        "a closed chat's screenshots are gone, and nothing is attached"
    );
}

#[test]
fn a_request_that_is_not_open_gets_no_proof_and_nothing_is_kept() {
    let dir = project();
    took(dir.path(), 1, b"\x89PNG one");
    let shown = show(dir.path(), CHAT, PR, &shots(dir.path())).expect("shown");
    let recorded = Arc::new(
        Recorded::parse(
            &json!({"source": "GitHub REST API version 2022-11-28, pulls: Get a pull request", "exchanges": [get(
                "repos/acme/api/pulls/12",
                json!({"number": 12, "state": "closed", "merged": true}),
            )]})
            .to_string(),
        )
        .expect("a recording"),
    );

    assert_eq!(
        attach(
            dir.path(),
            CHAT,
            &shown,
            &over(&recorded),
            &Caller::window()
        ),
        Err("#12 in acme/api is not open, so no proof is attached to it.".to_string())
    );
    assert_eq!(recorded.unspent(), Vec::new());
}
