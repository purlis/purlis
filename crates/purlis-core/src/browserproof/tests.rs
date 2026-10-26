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

/// The two reads [`show`] asks: request 12's state, and the repo's visibility and the person's
/// right to push there.
fn looked(state: &str, repo: Value) -> Vec<Value> {
    vec![
        get(
            "repos/acme/api/pulls/12",
            json!({"number": 12, "state": state, "merged": state == "closed"}),
        ),
        get("repos/acme/api", repo),
    ]
}

/// A private repo the person may push to.
fn pushable() -> Value {
    json!({"visibility": "private", "permissions": {"push": true, "pull": true}})
}

fn recorded(exchanges: Vec<Value>) -> Arc<Recorded> {
    Arc::new(
        Recorded::parse(
            &json!({"source": "GitHub REST API version 2022-11-28, as the forge contract \
                               recordings", "exchanges": exchanges})
            .to_string(),
        )
        .expect("a recording"),
    )
}

/// A GitHub whose request 12 is open and whose proof branch is not there yet, answering the
/// reads of a show, then the keeping of `pngs` and a comment of `body` answered at `page`.
fn github(pngs: &[&[u8]], body: &str, page: &str) -> Arc<Recorded> {
    let commit = "6".repeat(40);
    let mut exchanges = looked("open", pushable());
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
        json!({"html_url": page}),
    ));
    recorded(exchanges)
}

fn over(recorded: &Arc<Recorded>) -> impl Fn(&Forge) -> Result<Box<dyn ForgeBackend>, String> {
    let recorded = recorded.clone();
    move |forge: &Forge| Ok(forge.backend_over(recorded.clone()))
}

fn never(_: &Forge) -> Result<Box<dyn ForgeBackend>, String> {
    panic!("the forge was asked")
}

fn shown_now(root: &Path, picked: &[u64], repo: Value) -> Result<Showing, String> {
    let reads = recorded(looked("open", repo));
    let showing = show(root, CHAT, PR, picked, &over(&reads), &Caller::window());
    assert_eq!(reads.unspent(), Vec::new(), "both reads asked");
    showing
}

/// The comment the two screenshots make, kept on the first proof's commit.
fn body_of_two() -> String {
    format!(
        "{}\n\n![Browser screenshot 1](https://github.com/acme/api/raw/{c}/browser-proof-1.png)\
         \n\n![Browser screenshot 2](https://github.com/acme/api/raw/{c}/browser-proof-2.png)",
        words(2),
        c = "6".repeat(40)
    )
}

#[test]
fn only_the_screenshots_the_person_picked_are_shown_with_their_bytes_and_where_they_go() {
    let dir = project();
    took(dir.path(), 1, b"\x89PNG one");
    took(dir.path(), 2, b"\x89PNG two");
    let all = shots(dir.path());

    let showing = shown_now(dir.path(), &all[1..], pushable()).expect("shown");

    let shown = &showing.shown;
    assert_eq!(shown.request, PR);
    assert_eq!(shown.number, "#12");
    assert_eq!(shown.path, "acme/api");
    assert_eq!(
        shown.kept_at,
        "on the branch purlis/browser-proof/pr-12 of acme/api, in its history for good"
    );
    assert_eq!(
        shown.shots,
        [ShownShot {
            shot: all[1],
            at: 2,
            digest: "4ce1f4445c0247b6b59c2c6abdc3f68d4ecf5f165db2a0aaa9c9edb6f429c94e".into(),
        }]
    );
    assert_eq!(showing.pictures, [b"\x89PNG two".to_vec()]);
    assert!(
        shown
            .words
            .starts_with("### Browser proof\n\n1 screenshot of")
    );
}

/// The person's ruling on D-2026-3: a line before the pictures says they are public and
/// permanent to the repo's readers, and says "public" plainly when the repo is.
#[test]
fn the_warning_says_public_and_permanent_and_whether_the_repo_is_public() {
    let dir = project();
    took(dir.path(), 1, b"\x89PNG one");
    let public = json!({"visibility": "public", "permissions": {"push": true}});

    let shown = shown_now(dir.path(), &shots(dir.path()), public)
        .expect("shown")
        .shown;

    assert_eq!(
        shown.warning,
        "Public and permanent: anyone who can read acme/api (everyone, on a public repo) sees \
         these, even after the request closes. acme/api is public: everyone can see these."
    );
    let private = shown_now(dir.path(), &shots(dir.path()), pushable()).expect("shown");
    assert!(private.shown.warning.ends_with("acme/api is private."));
}

#[test]
fn someone_who_cannot_push_to_the_repo_is_told_before_anything_is_written() {
    let dir = project();
    took(dir.path(), 1, b"\x89PNG one");
    let fork = json!({"visibility": "public", "permissions": {"push": false, "pull": true}});

    assert_eq!(
        shown_now(dir.path(), &shots(dir.path()), fork),
        Err(
            "You cannot push to acme/api, and the pictures are kept on a branch there, so no \
             proof can be attached to #12. Someone who can push to acme/api can attach it."
                .to_string()
        )
    );
}

#[test]
fn nothing_picked_or_a_screenshot_no_longer_kept_is_refused() {
    let dir = project();
    took(dir.path(), 1, b"\x89PNG one");
    let refused = |chat: &str, picked: &[u64]| {
        show(dir.path(), chat, PR, picked, &never, &Caller::window()).map(|_| ())
    };
    assert_eq!(
        refused(CHAT, &[]),
        Err("Pick at least one screenshot to attach.".to_string())
    );
    assert_eq!(refused(CHAT, &[999]), Err(NO_LONGER_KEPT.to_string()));
    assert_eq!(
        refused("01JB0000000000000000000001", &shots(dir.path())),
        Err(NO_LONGER_KEPT.to_string()),
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
    let shown = shown_now(dir.path(), &all, pushable())
        .expect("shown")
        .shown;
    let body = body_of_two();
    assert!(!body.contains("staging") && !body.contains("agent-typed"));
    let page = "https://github.com/acme/api/pull/12#issuecomment-1";
    let forge = github(&[b"\x89PNG one", b"\x89PNG two"], &body, page);

    let attached = attach(dir.path(), CHAT, &shown, &over(&forge), &Caller::window());

    assert_eq!(
        attached,
        Ok(Attached {
            comment: page.into()
        })
    );
    assert_eq!(forge.unspent(), Vec::new(), "every exchange asked");
}

/// A comment's page the forge answers off its own host is not handed to the window to open:
/// the request's own page is, which the person gave.
#[test]
fn a_comment_page_off_the_forges_host_is_never_the_one_opened() {
    let dir = project();
    took(dir.path(), 1, b"\x89PNG one");
    took(dir.path(), 2, b"\x89PNG two");
    let shown = shown_now(dir.path(), &shots(dir.path()), pushable())
        .expect("shown")
        .shown;
    let forge = github(
        &[b"\x89PNG one", b"\x89PNG two"],
        &body_of_two(),
        "file:///etc/passwd",
    );

    let attached = attach(dir.path(), CHAT, &shown, &over(&forge), &Caller::window());

    assert_eq!(attached, Ok(Attached { comment: PR.into() }));
}

#[test]
fn a_shown_proof_that_changed_since_is_refused_before_anything_is_written() {
    let dir = project();
    took(dir.path(), 1, b"\x89PNG one");
    let shown = shown_now(dir.path(), &shots(dir.path()), pushable())
        .expect("shown")
        .shown;

    let mut other = shown.clone();
    other.shots[0].digest = "0".repeat(64);
    let mut reworded = shown.clone();
    reworded.words = "Typed by the agent".into();
    let mut quieter = shown.clone();
    quieter.warning = "Nobody will see these.".into();
    for changed in [other, reworded, quieter] {
        let reads = recorded(looked("open", pushable()));
        assert_eq!(
            attach(dir.path(), CHAT, &changed, &over(&reads), &Caller::window()),
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
    let shown = shown_now(dir.path(), &shots(dir.path()), pushable())
        .expect("shown")
        .shown;
    let closed = recorded(vec![get(
        "repos/acme/api/pulls/12",
        json!({"number": 12, "state": "closed", "merged": true}),
    )]);

    assert_eq!(
        attach(dir.path(), CHAT, &shown, &over(&closed), &Caller::window()),
        Err("#12 in acme/api is not open, so no proof is attached to it.".to_string())
    );
    assert_eq!(closed.unspent(), Vec::new());
}
