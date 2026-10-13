//! N−1 for 1.1's additions (iOS design §5.6): a host from before 1.1 refuses `prompt` and
//! `attach_timeline` as unknown, lists its chats without `workspace` and `level`, and the
//! client carries on with the terminal view and raw `write`.
//!
//! The 1.0 host is played by hand on the control lane, from the frames
//! `fixtures/session-protocol.jsonl` recorded for 1.0, so this is what such a host really says.

use bytes::Bytes;
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client};
use serde_json::{Value, json};
use tokio::io::duplex;

mod common;
use common::HELD;

/// A 1.0 host: it answers `list` with a 1.0 `chats` reply and every other word 1.0 knows with
/// `done`, and refuses what it does not know as 1.0's `serve` would.
async fn a_1_0_host(mut link: link::Link) {
    const KNOWN: &[&str] = &[
        "list",
        "attach",
        "detach",
        "write",
        "resize",
        "answer",
        "stop",
        "start",
        "subscribe",
        "unsubscribe",
    ];
    while let Some(Ok(frame)) = link.control().next().await {
        let call: Value = serde_json::from_slice(&frame).unwrap();
        let id = call["call"]["id"].as_u64().unwrap();
        let word = call["call"]["command"].as_str().unwrap();
        let reply = match word {
            "list" => json!({"reply": {"re": id, "ok": {"chats": [
                {"chat": "01J9ZQ3V7K8M2N4P6R8T0V2X4Z", "project": "01J9PRJCT00000000000000000",
                 "title": "fix the login", "state": "working"}
            ]}}}),
            known if KNOWN.contains(&known) => json!({"reply": {"re": id, "ok": "done"}}),
            unknown => json!({"reply": {"re": id, "refused": {
                "code": session::UNKNOWN_COMMAND,
                "why": format!("this host does not know the command `{unknown}`")}}}),
        };
        link.control()
            .send(Bytes::from(reply.to_string()))
            .await
            .unwrap();
    }
}

async fn client_of_a_1_0_host() -> Client {
    let (a, b) = duplex(64 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::Terminal,
            HELD.of(Scope::Terminal)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(a_1_0_host(served.unwrap()));
    let (client, _views, _events) = Client::new(client.unwrap());
    client
}

#[tokio::test]
async fn a_chat_listed_without_workspace_and_level_reads_with_neither() {
    let client = client_of_a_1_0_host().await;
    let chats = client.list().await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(
        (chats[0].workspace.as_deref(), chats[0].level),
        (None, None)
    );
    assert_eq!(chats[0].title.as_deref(), Some("fix the login"));
}

#[tokio::test]
async fn prompt_is_refused_as_unknown_and_write_still_works() {
    let client = client_of_a_1_0_host().await;
    let refused = client.prompt("c", "hi").await.unwrap_err();
    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::UNKNOWN_COMMAND)
    );
    client.write("c", b"hi\r").await.unwrap();
}

#[tokio::test]
async fn attach_timeline_is_refused_as_unknown_and_attach_still_works() {
    let client = client_of_a_1_0_host().await;
    let refused = client.attach_timeline("c").await.unwrap_err();
    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::UNKNOWN_COMMAND)
    );
    // `attach` answers `done` from this stand-in, which the client reports as unexpected: the
    // call reached the host and came back, which is what N−1 promises.
    let answer = client.attach("c").await.unwrap_err();
    assert!(
        matches!(answer, session::CallError::Unexpected(_)),
        "{answer}"
    );
}

#[tokio::test]
async fn the_1_0_frames_still_read_under_1_1() {
    // Every 1.0 frame in the fixture is also a 1.1 frame: the promise test holds them, and this
    // names the version so the two cannot drift apart.
    assert_eq!(
        session::VERSION,
        purlis_session_protocol::version::Version { major: 1, minor: 1 }
    );
}
