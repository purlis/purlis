//! Pairing from the phone's side (iOS design §4.2), against the protocol crate's own gateway
//! in process: the scanned code, the handshake against the scanned key, the one proof frame,
//! and the desktop's verdict. The desk confirms, or does not; a wrong code pins nothing; a
//! code naming another key is refused before any frame.

mod support;

use std::sync::Arc;

use purlis_mobile_core::Error;
use purlis_mobile_core::hosts;
use purlis_mobile_core::identity::{self, Device};
use purlis_mobile_core::link::Link;
use purlis_mobile_core::pair::pair;
use purlis_mobile_core::payload::Payload;
use purlis_mobile_core::store::MemoryStore;
use purlis_session_protocol::gateway::Heard;
use purlis_session_protocol::noise::Identity;
use purlis_session_protocol::pairing::Code;
use support::{HOST, hex, rig};

fn a_phone() -> (MemoryStore, Identity, Device) {
    let store = MemoryStore::default();
    let me = identity::link_key(&store).unwrap();
    let device = identity::device(&store, Some("Noro's iPhone")).unwrap();
    (store, me, device)
}

#[tokio::test]
async fn a_confirmed_pairing_pins_the_desktop_with_the_scanned_key_and_addresses() {
    let rig = Arc::new(rig(true));
    let (store, me, device) = a_phone();
    let payload = rig.payload(vec!["192.168.9.155:7777".into(), "100.64.0.9:7777".into()]);

    let mut pinned = pair(rig.phone_side(), &me, &device, &payload)
        .await
        .expect("the person confirmed");
    assert_eq!(pinned.device, HOST);
    assert_eq!(pinned.key, rig.identity.public());
    assert_eq!(pinned.addresses, payload.addresses);
    assert!(pinned.paired > 0);
    // The desktop was offered this phone, and pinned it.
    assert!(
        rig.heard
            .lock()
            .unwrap()
            .iter()
            .any(|h| matches!(h, Heard::Paired { .. }))
    );

    pinned.name = "Mac".into();
    hosts::add(&store, pinned.clone()).unwrap();
    assert_eq!(hosts::find(&store, HOST).unwrap(), Some(pinned.clone()));

    // From now on the phone links with its key alone, and the desktop lists its chats.
    let link = Link::open(rig.phone_side(), &me, &pinned).await.unwrap();
    let chats = link.list().await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].chat, support::KEPT);
}

#[tokio::test]
async fn a_pairing_the_person_did_not_confirm_is_refused_and_pins_nothing() {
    let rig = Arc::new(rig(false));
    let (_store, me, device) = a_phone();
    let payload = rig.payload(vec![]);
    let refused = pair(rig.phone_side(), &me, &device, &payload)
        .await
        .unwrap_err();
    assert!(
        matches!(refused, Error::PairingRefused(ref why) if why.contains("not confirmed")),
        "{refused}"
    );
    // And the phone's key is not pinned: a link with it is dropped.
    assert!(rig.peers.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_wrong_code_is_refused_by_the_desktop() {
    let rig = Arc::new(rig(true));
    let (_store, me, device) = a_phone();
    let mut payload = rig.payload(vec![]);
    payload.code = Code::mint().to_hex();
    let refused = pair(rig.phone_side(), &me, &device, &payload)
        .await
        .unwrap_err();
    assert!(matches!(refused, Error::PairingRefused(_)), "{refused}");
    assert!(rig.peers.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_code_that_names_another_desktops_key_is_refused_before_any_frame() {
    let rig = Arc::new(rig(true));
    let (_store, me, device) = a_phone();
    let mut payload = rig.payload(vec![]);
    payload.host_key = hex(&Identity::generate().public());
    let refused = pair(rig.phone_side(), &me, &device, &payload)
        .await
        .unwrap_err();
    assert!(matches!(refused, Error::WrongKey), "{refused}");
    // The phone sent nothing past the handshake: the gateway heard no link, no proof and no
    // refusal of one, and the window's code was not spent on it.
    assert!(
        rig.heard.lock().unwrap().is_empty(),
        "{:?}",
        rig.heard.lock().unwrap()
    );
}

#[test]
fn the_pairing_code_is_read_from_scanned_or_pasted_text_and_never_printed() {
    let rig = rig(true);
    let text = rig.payload(vec!["10.0.0.5:7777".into()]).to_qr_text();
    let read = Payload::from_qr_text(&format!("  {text}\n")).unwrap();
    assert_eq!(read.host_device, HOST);
    assert_eq!(read.addresses, vec!["10.0.0.5:7777".to_owned()]);
    assert_eq!(read.host_key().unwrap(), rig.identity.public());
    assert!(
        !format!("{read:?}").contains(&read.code),
        "the code is not in a log line"
    );
    assert!(matches!(
        Payload::from_qr_text("hello"),
        Err(Error::Payload(_))
    ));
    assert!(matches!(
        Payload::from_qr_text("purlis-pair:{"),
        Err(Error::Payload(_))
    ));
    assert!(matches!(
        Payload::from_qr_text(
            "purlis-pair:{\"version\":2,\"host_device\":\"x\",\"host_key\":\"\",\"addresses\":[],\"code\":\"\"}"
        ),
        Err(Error::Payload(_))
    ));
}
