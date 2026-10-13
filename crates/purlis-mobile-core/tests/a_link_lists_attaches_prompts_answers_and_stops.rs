//! A link to a desktop (iOS design §4.3, §5): what a pinned phone may do, each reaching the
//! desktop as the command it is; the streams the desktop opens, routed to the view or
//! timeline that attached them; and a link that drops, which ends every reader and refuses
//! every call at once.

mod support;

use std::sync::Arc;
use std::time::Duration;

use purlis_mobile_core::Error;
use purlis_mobile_core::identity;
use purlis_mobile_core::link::Link;
use purlis_mobile_core::store::MemoryStore;
use purlis_session_protocol::session::Command;
use purlis_session_protocol::timeline::What;
use purlis_session_protocol::view::Chunk;
use support::{KEPT, rig};

async fn linked() -> (Arc<support::Rig>, Link) {
    let rig = Arc::new(rig(true));
    let store = MemoryStore::default();
    let me = identity::link_key(&store).unwrap();
    rig.pin(me.public());
    let link = Link::open(rig.phone_side(), &me, &rig.pinned())
        .await
        .unwrap();
    (rig, link)
}

async fn within<T>(what: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), what)
        .await
        .expect("within five seconds")
}

#[tokio::test]
async fn list_names_the_desktops_chats_with_their_workspace_and_level() {
    let (_rig, link) = linked().await;
    let chats = link.list().await.unwrap();
    assert_eq!(chats, vec![support::the_chat()]);
    assert_eq!(chats[0].workspace.as_deref(), Some("login"));
    assert_eq!(chats[0].level, Some(2));
}

#[tokio::test]
async fn a_view_brings_the_screen_then_the_live_bytes_and_detach_closes_it() {
    let (rig, link) = linked().await;
    let (attached, mut chunks, acks) = link.attach(KEPT).await.unwrap();
    assert_eq!((attached.cols, attached.rows), (80, 24));
    let first = within(chunks.recv()).await.expect("the screen");
    assert!(
        matches!(&first, Chunk::Snapshot(b) if &b[..] == b"screen\r\n"),
        "{first:?}"
    );
    // The terminal drew it: the host hears so, and sends on.
    acks.send(b"screen\r\n".len()).unwrap();
    let second = within(chunks.recv()).await.expect("the live line");
    assert!(
        matches!(&second, Chunk::Live(b) if &b[..] == b"live\r\n"),
        "{second:?}"
    );
    link.detach(KEPT, attached.view).await.unwrap();
    assert!(
        rig.host
            .calls()
            .iter()
            .any(|c| matches!(c, Command::Detach { view: 1, .. }))
    );
}

#[tokio::test]
async fn a_timeline_brings_the_snapshot_then_caught_up_then_what_follows() {
    let (_rig, link) = linked().await;
    let (attached, mut items) = link.attach_timeline(KEPT).await.unwrap();
    assert_eq!(attached.level, 2);
    let mut whats = Vec::new();
    for _ in 0..4 {
        whats.push(within(items.recv()).await.expect("an item").what);
    }
    assert!(matches!(&whats[0], What::Turn { began: true, .. }));
    assert!(
        matches!(&whats[1], What::Ask { ask, choices, .. } if ask == "a1" && choices.iter().any(|c| c.lasts))
    );
    assert_eq!(whats[2], What::CaughtUp);
    assert!(matches!(&whats[3], What::AskClosed { ask, by } if ask == "a1" && by == "desktop"));
    link.detach_timeline(KEPT, attached.timeline).await.unwrap();
}

#[tokio::test]
async fn write_prompt_answer_and_stop_reach_the_desktop_as_the_commands_they_are() {
    let (rig, link) = linked().await;
    link.write(KEPT, b"\x1b").await.unwrap();
    link.prompt(KEPT, "also add a test for expiry")
        .await
        .unwrap();
    link.answer(KEPT, "a1", "allow").await.unwrap();
    link.stop(KEPT).await.unwrap();
    let calls = rig.host.calls();
    assert!(calls.contains(&Command::Write {
        chat: KEPT.into(),
        bytes: b"\x1b".to_vec()
    }));
    assert!(calls.contains(&Command::Prompt {
        chat: KEPT.into(),
        text: "also add a test for expiry".into()
    }));
    assert!(calls.contains(&Command::Answer {
        chat: KEPT.into(),
        ask: "a1".into(),
        answer: "allow".into()
    }));
    assert!(calls.contains(&Command::Stop { chat: KEPT.into() }));
}

#[tokio::test]
async fn a_lasting_choice_is_refused_by_the_desktop_not_hidden_by_the_link() {
    let (_rig, link) = linked().await;
    let refused = link.answer(KEPT, "a1", "always").await.unwrap_err();
    assert!(matches!(refused, Error::Refused(_)), "{refused}");
}

#[tokio::test]
async fn a_view_or_timeline_the_desktop_ends_ends_for_the_app_while_the_link_lives() {
    let (rig, link) = linked().await;
    let (_view, mut chunks, _acks) = link.attach(KEPT).await.unwrap();
    let (_timeline, mut items) = link.attach_timeline(KEPT).await.unwrap();
    within(chunks.recv()).await.expect("the screen");
    within(items.recv()).await.expect("an item");
    // The desktop closes both (a view that fell behind is closed by its feed): the app's
    // receivers end, so it knows to attach again — and the link itself is still up.
    rig.host.end_streams();
    within(async { while chunks.recv().await.is_some() {} }).await;
    within(async { while items.recv().await.is_some() {} }).await;
    assert!(!link.is_closed());
    assert_eq!(link.list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_link_that_drops_ends_its_readers_and_refuses_every_call_at_once() {
    let (rig, link) = linked().await;
    let (_attached, mut chunks, _acks) = link.attach(KEPT).await.unwrap();
    within(chunks.recv()).await.expect("the screen");
    rig.desktop_goes_away();
    // The reader ends.
    within(async { while chunks.recv().await.is_some() {} }).await;
    // And the next call fails now, with nothing queued.
    let failed = within(async {
        loop {
            match link.list().await {
                Ok(_) => tokio::time::sleep(Duration::from_millis(20)).await,
                Err(err) => break err,
            }
        }
    })
    .await;
    assert!(matches!(failed, Error::NotConnected), "{failed}");
    assert!(link.is_closed());
}
