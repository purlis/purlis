//! Where the gateway listens and how its desk, pins and revocations sit on core's state
//! (iOS design §3, §4.2): what a start binds and never binds, the adapters over the desk, and
//! a listener a phone links to and that a stop takes down.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use purlis_core::remote::linkkey::LinkKey;
use purlis_core::remote::pairing::Desk;
use purlis_core::remote::peers::{self, Peer};
use purlis_session_protocol::gateway::{Offered, PairingDesk};

use super::adapters::{DeskAdapter, revocations};
use super::gateway::{Remote, bind_addresses, primary_address};

const HOST: &str = "01J9DEVCE00000000000000000";
const PHONE: &str = "01J9PHONE00000000000000000";

#[test]
fn the_listener_binds_each_address_it_can_and_never_everything() {
    // 0.0.0.0 is refused outright; text that is not an address is skipped and reported; the
    // port is the configured one unless the address carries its own.
    let planned = bind_addresses(
        Some(Ipv4Addr::new(192, 168, 9, 155)),
        &[
            "0.0.0.0".into(),
            "100.64.0.9".into(),
            "not an address".into(),
            "10.0.0.5:7800".into(),
            "100.64.0.9".into(),
        ],
        7777,
    );
    assert_eq!(
        planned.bind,
        vec![
            SocketAddr::from(([192, 168, 9, 155], 7777)),
            SocketAddr::from(([100, 64, 0, 9], 7777)),
            SocketAddr::from(([10, 0, 0, 5], 7800)),
        ]
    );
    assert_eq!(
        planned.skipped,
        vec![
            ("0.0.0.0".to_owned(), "never every interface".to_owned()),
            ("not an address".to_owned(), "not an address".to_owned()),
        ]
    );
}

#[test]
fn with_no_primary_address_only_the_configured_ones_are_bound() {
    let planned = bind_addresses(None, &["100.64.0.9".into()], 7777);
    assert_eq!(
        planned.bind,
        vec![SocketAddr::from(([100, 64, 0, 9], 7777))]
    );
    assert!(bind_addresses(None, &[], 7777).bind.is_empty());
}

#[test]
fn the_primary_address_is_never_loopback_or_unspecified() {
    if let Some(ip) = primary_address() {
        assert!(!ip.is_loopback() && !ip.is_unspecified(), "{ip}");
    }
}

#[tokio::test]
async fn the_desk_adapter_takes_once_and_withdraws_by_key() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Arc::new(Desk::new(dir.path().to_path_buf()));
    let host = LinkKey::from_private([9u8; 32]);
    desk.open_window(&host, HOST, vec![], Instant::now());
    let adapter = DeskAdapter::new(desk.clone());
    assert!(adapter.take().is_some());
    assert!(adapter.take().is_none(), "one try per code");
    let offer = Offered {
        device: PHONE.into(),
        name: "phone".into(),
        key: [1u8; 32],
        fingerprint: "f".into(),
    };
    let verdict = adapter.offered(offer.clone());
    assert!(desk.pending().is_some(), "the window is asked");
    adapter.withdraw(&offer);
    assert!(desk.pending().is_none(), "withdrawn by key");
    drop(verdict);
}

#[tokio::test]
async fn a_revocation_on_the_desk_bumps_the_watch() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Arc::new(Desk::new(dir.path().to_path_buf()));
    let mut revoked = revocations(&desk);
    peers::add(
        dir.path(),
        Peer {
            device: PHONE.into(),
            name: "p".into(),
            key: [1u8; 32],
            paired: 1,
        },
    )
    .unwrap();
    desk.unpair(PHONE).unwrap();
    assert!(revoked.has_changed().unwrap());
    assert_eq!(*revoked.borrow_and_update(), desk.generation());
}

#[test]
fn stopping_a_pairing_closes_the_window_so_no_phone_can_take_it() {
    let config = tempfile::tempdir().unwrap();
    let remote = Remote::new(Some(config.path().to_path_buf()), None)
        .with_key(LinkKey::from_private([9u8; 32]));
    let desk = remote.desk_for_tests();
    desk.open_window(
        &LinkKey::from_private([9u8; 32]),
        HOST,
        vec![],
        Instant::now(),
    );
    assert!(desk.window(Instant::now()).is_some());
    // A phone proved the code and waits on the person: a stop answers it too.
    let ticket = desk.offer(
        purlis_core::remote::pairing::Offer {
            device: PHONE.into(),
            name: "phone".into(),
            key: [1u8; 32],
            fingerprint: "f".into(),
        },
        Instant::now() + Duration::from_secs(60),
    );
    remote.pair_stop();
    assert!(
        desk.window(Instant::now()).is_none(),
        "the window is closed"
    );
    assert!(desk.pending().is_none(), "the offer is no longer waiting");
    assert!(
        matches!(
            desk.take_verdict(ticket),
            Some(purlis_core::remote::pairing::Verdict::Refused { .. })
        ),
        "the phone is told it was not confirmed"
    );
}

/// A listener on loopback, a phone's link through the real handshake, and a stop that takes
/// the port down. Binds a port, which the chat's sandbox refuses: run outside it.
#[tokio::test]
async fn a_started_listener_serves_a_pinned_phone_and_a_stop_takes_it_down() {
    use purlis_session_protocol::auth::Scope;
    use purlis_session_protocol::link;
    use purlis_session_protocol::noise::{self, Identity};
    use purlis_session_protocol::session::{self, Client};

    let config = tempfile::tempdir().unwrap();
    purlis_core::machine::update(config.path(), |store| {
        store.gateway.addresses = vec!["127.0.0.1:0".into()];
    })
    .unwrap();
    let host_key = LinkKey::from_private([9u8; 32]);
    let remote = Remote::new(Some(config.path().to_path_buf()), None)
        .with_key(host_key.clone())
        .only_configured_addresses();
    let host_device = remote.device().unwrap();
    let phone = Identity::generate();
    peers::add(
        config.path(),
        Peer {
            device: PHONE.into(),
            name: "phone".into(),
            key: phone.public(),
            paired: 1,
        },
    )
    .unwrap();

    let planes = Arc::new(crate::planes::Planes::telling(
        Arc::new(|_: crate::hooks::Moved| {}),
        crate::Shipped::default(),
        None,
    ));
    let bound = remote.start(Arc::new(planes)).unwrap();
    assert_eq!(bound.len(), 1);
    assert!(bound[0].ip().is_loopback() && bound[0].port() != 0);
    assert_eq!(remote.bound(), bound);

    let tcp = tokio::net::TcpStream::connect(bound[0]).await.unwrap();
    let secured = noise::connect(tcp, &phone, &host_device, Some(host_key.public))
        .await
        .unwrap();
    let plain = noise::pump(secured);
    let linked = link::connect_as_a_device(plain, session::speaks(), Scope::RemoteDevice)
        .await
        .unwrap();
    let (client, _views, _events) = Client::new(linked);
    assert_eq!(client.list().await.unwrap(), vec![], "no plane is open");

    remote.stop();
    assert!(remote.bound().is_empty());
    let refused = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::TcpStream::connect(bound[0]),
    )
    .await
    .expect("answered");
    assert!(refused.is_err(), "the port is down");
    // And the phone that was linked is cut off, not only new ones (iOS design §4.2: turning
    // access off ends every link).
    let after = tokio::time::timeout(Duration::from_secs(5), client.list()).await;
    assert!(
        matches!(after, Ok(Err(_))),
        "the live link ended: {after:?}"
    );
}

/// Review Focus 5: the assertion follows the chats across the planes — held while one works
/// and the setting is on, released when the chat stops, and released when the setting goes.
#[test]
fn the_machine_is_held_awake_only_while_a_chat_works_and_the_setting_is_on() {
    use super::awake::Program;
    use super::board_tests::{KEPT, a_plane_recording, held_of, start};
    use crate::host::pretend::Pretend;
    use purlis_core::hookwire::{Conversation, Report};
    use purlis_core::state::{Detail, Event};

    let config = tempfile::tempdir().unwrap();
    purlis_core::machine::update(config.path(), |store| store.keep_awake = true).unwrap();
    let remote = Remote::new(Some(config.path().to_path_buf()), None)
        .holding_awake_with(Program::test_double());
    let pretend = Pretend::default();
    let planes = a_plane_recording(&pretend, None);
    let session = start(&planes, "/nowhere/claude", "one", Some(KEPT), true);
    let held = held_of(&planes);
    // A Claude Code chat adopts its harness by the pid and conversation its first report
    // names, and takes the rest from that pid.
    let report = |event: Event| Report {
        chat: session,
        event,
        conversation: Conversation::Named("11111111-2222-4333-8444-555555555555".into()),
        pid: Some(4242),
        agent: None,
        detail: Detail::default(),
    };

    remote.reconsider_awake(&planes);
    assert!(!remote.holds_awake(), "an idle chat holds nothing");

    // The hook board hears of the chat as the plane's start tells it, then its reports.
    held.hooks().board().opened(
        session,
        Some(purlis_core::harness::Harness::ClaudeCode),
        None,
    );
    assert!(held.hooks().board().reported(&report(Event::SessionStart)));
    remote.reconsider_awake(&planes);
    assert!(
        !remote.holds_awake(),
        "a chat that started and waits holds nothing"
    );
    assert!(
        held.hooks()
            .board()
            .reported(&report(Event::UserPromptSubmit))
    );
    assert_eq!(
        held.board().glance(session).state,
        purlis_core::state::State::Running
    );
    remote.reconsider_awake(&planes);
    assert!(
        remote.holds_awake(),
        "a working chat holds the machine awake"
    );

    held.hooks().board().reported(&report(Event::Stop));
    remote.reconsider_awake(&planes);
    assert!(
        !remote.holds_awake(),
        "a chat waiting on the person releases it"
    );

    held.hooks()
        .board()
        .reported(&report(Event::UserPromptSubmit));
    remote.reconsider_awake(&planes);
    assert!(remote.holds_awake());
    remote.remember_keep_awake(false);
    remote.reconsider_awake(&planes);
    assert!(
        !remote.holds_awake(),
        "the setting off releases it whatever the chats do"
    );

    remote.remember_keep_awake(true);
    remote.reconsider_awake(&planes);
    assert!(remote.holds_awake());
    remote.release_awake();
    assert!(!remote.holds_awake(), "quitting releases it");
}
