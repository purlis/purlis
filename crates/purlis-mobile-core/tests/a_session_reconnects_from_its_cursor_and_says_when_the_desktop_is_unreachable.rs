//! A desktop across reconnects (iOS design §6.2, §6.3): Connected, Reconnecting…, Desktop
//! unreachable; a link that dropped is connected again on asking, with what was open to
//! attach again; the `subscribe(since)` cursor kept per desktop; and a desktop whose key is
//! not the pinned one, which is unreachable and never re-paired.

mod support;

use std::sync::Arc;
use std::time::Duration;

use purlis_mobile_core::Error;
use purlis_mobile_core::hosts::{self, Host};
use purlis_mobile_core::identity;
use purlis_mobile_core::session::{Open, Session, Standing};
use purlis_mobile_core::store::{MemoryStore, Store};
use purlis_session_protocol::noise::Identity;
use purlis_session_protocol::session::Event;
use support::{DialTheRig, HOST, KEPT, NoDial, rig};

fn a_phone() -> (Arc<MemoryStore>, Identity) {
    let store = Arc::new(MemoryStore::default());
    let me = identity::link_key(&*store).unwrap();
    (store, me)
}

async fn within<T>(what: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), what)
        .await
        .expect("within five seconds")
}

fn event(device: &str, seq: u64) -> Event {
    Event {
        v: 1,
        device_id: device.into(),
        seq,
        ulid: format!("01J9EVENT{seq:017}"),
        chat: Some(KEPT.into()),
        run: None,
        parent_run: None,
        kind: "hook.stop".into(),
        body: serde_json::json!({}),
    }
}

#[tokio::test]
async fn a_session_connects_again_after_the_desktop_went_away_and_knows_what_to_reattach() {
    let rig = Arc::new(rig(true));
    let (store, me) = a_phone();
    rig.pin(me.public());
    let session = Session::new(
        store,
        me,
        rig.pinned(),
        Arc::new(DialTheRig(Arc::clone(&rig))),
    );
    let mut standing = session.standing();
    assert_eq!(*standing.borrow(), Standing::Reconnecting);
    assert!(matches!(session.link(), Err(Error::NotConnected)));

    session.connect().await.unwrap();
    assert_eq!(*standing.borrow_and_update(), Standing::Connected);
    let link = session.link().unwrap();
    assert_eq!(link.list().await.unwrap().len(), 1);
    session.opened(Open::View { chat: KEPT.into() });
    session.opened(Open::Timeline { chat: KEPT.into() });
    session.opened(Open::View { chat: KEPT.into() });

    // The phone went to the background: iOS drops the link. The standing says so by itself,
    // before anything asks for the link.
    rig.desktop_goes_away();
    within(standing.changed()).await.unwrap();
    assert_eq!(*standing.borrow_and_update(), Standing::Reconnecting);
    assert!(matches!(session.link(), Err(Error::NotConnected)));

    // Back in the foreground: a new link, and what was open is listed to attach again.
    session.connect().await.unwrap();
    assert_eq!(*standing.borrow_and_update(), Standing::Connected);
    assert_eq!(session.link().unwrap().list().await.unwrap().len(), 1);
    assert_eq!(
        session.to_reattach(),
        vec![
            Open::View { chat: KEPT.into() },
            Open::Timeline { chat: KEPT.into() }
        ]
    );
    session.closed(&Open::View { chat: KEPT.into() });
    assert_eq!(
        session.to_reattach(),
        vec![Open::Timeline { chat: KEPT.into() }]
    );
}

#[tokio::test]
async fn a_desktop_no_address_answers_for_is_unreachable_with_why() {
    let (store, me) = a_phone();
    let host = Host {
        addresses: vec!["192.168.9.155:7777".into(), "100.64.0.9:7777".into()],
        ..rig(true).pinned()
    };
    let session = Session::new(store, me, host, Arc::new(NoDial));
    let failed = session.connect().await.unwrap_err();
    assert!(matches!(failed, Error::Unreachable(_)), "{failed}");
    match &*session.standing().borrow() {
        Standing::Unreachable(why) => assert!(why.contains("connection refused"), "{why}"),
        other => panic!("{other:?}"),
    }
    assert!(matches!(session.link(), Err(Error::NotConnected)));
}

#[tokio::test]
async fn a_desktop_with_another_key_is_refused_not_re_paired() {
    let rig = Arc::new(rig(true));
    let (store, me) = a_phone();
    rig.pin(me.public());
    // The phone pinned a key the desktop no longer has (it rotated its link key).
    let pinned = Host {
        key: Identity::generate().public(),
        ..rig.pinned()
    };
    hosts::add(&*store, pinned.clone()).unwrap();
    let session = Session::new(
        Arc::clone(&store) as Arc<dyn Store>,
        me,
        pinned.clone(),
        Arc::new(DialTheRig(Arc::clone(&rig))),
    );
    let failed = session.connect().await.unwrap_err();
    assert!(matches!(failed, Error::WrongKey), "{failed}");
    assert_eq!(
        *session.standing().borrow(),
        Standing::Unreachable("the desktop's key is not the one you paired with".into())
    );
    assert_eq!(
        hosts::find(&*store, HOST).unwrap(),
        Some(pinned),
        "nothing re-paired"
    );
}

#[tokio::test]
async fn the_cursor_is_kept_per_desktop_and_the_subscription_resumes_from_it() {
    let rig = Arc::new(rig(true));
    let (store, me) = a_phone();
    rig.pin(me.public());
    let session = Session::new(
        Arc::clone(&store) as Arc<dyn Store>,
        me.clone(),
        rig.pinned(),
        Arc::new(DialTheRig(Arc::clone(&rig))),
    );
    session.connect().await.unwrap();
    assert!(session.cursor().is_empty());
    let mut events = session.subscribe().await.unwrap();
    assert_eq!(
        rig.host.since.lock().unwrap().clone(),
        Some(Default::default())
    );

    rig.host.push(event(HOST, 1)).await;
    rig.host.push(event(HOST, 2)).await;
    for _ in 0..2 {
        if let Some(purlis_session_protocol::session::Pushed::Event(e)) =
            within(events.recv()).await
        {
            session.saw(&e).unwrap();
        }
    }
    assert_eq!(session.cursor().get(HOST).copied(), Some(2));

    // A new session over the same store, after a relaunch, resumes from the cursor.
    let again = Session::new(
        Arc::clone(&store) as Arc<dyn Store>,
        me,
        rig.pinned(),
        Arc::new(DialTheRig(Arc::clone(&rig))),
    );
    assert_eq!(again.cursor().get(HOST).copied(), Some(2));
    again.connect().await.unwrap();
    let _events = again.subscribe().await.unwrap();
    let since = rig.host.since.lock().unwrap().clone().unwrap();
    assert_eq!(
        since.get(HOST).copied(),
        Some(2),
        "subscribed from the cursor"
    );

    // Another desktop keeps its own cursor.
    let other = Session::new(
        Arc::clone(&store) as Arc<dyn Store>,
        identity::link_key(&*store).unwrap(),
        Host {
            device: "01J9LAPTOP0000000000000000".into(),
            ..rig.pinned()
        },
        Arc::new(NoDial),
    );
    assert!(other.cursor().is_empty());
}
