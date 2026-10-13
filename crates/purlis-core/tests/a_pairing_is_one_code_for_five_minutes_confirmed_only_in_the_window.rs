//! Pairing (iOS design §4.2): one code, five minutes, single use; a proof is offered to the
//! window, which alone confirms; a confirmation pins, a cancel or a window that ends pins
//! nothing; unpairing removes the record and tells every live link to re-check its key.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_core::remote::linkkey::LinkKey;
use purlis_core::remote::pairing::{Desk, Offer, Payload, Ticket, Verdict, WINDOW};
use purlis_core::remote::peers;

fn soon() -> Instant {
    Instant::now() + WINDOW
}

const HOST: &str = "01J9DEVCE00000000000000000";
const PHONE: &str = "01J9PHONE00000000000000000";

fn host() -> LinkKey {
    LinkKey::from_private([9u8; 32])
}

fn offer(key: u8) -> Offer {
    Offer {
        device: PHONE.into(),
        name: "Noro's iPhone".into(),
        key: [key; 32],
        fingerprint: purlis_core::remote::fingerprint(&[key; 32]),
    }
}

#[test]
fn a_window_is_one_code_that_the_qr_text_carries_with_the_hosts_key_and_addresses() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    let now = Instant::now();
    let payload = desk.open_window(&host(), HOST, vec!["192.168.9.155:7777".into()], now);
    assert_eq!(payload.host_device, HOST);
    assert_eq!(payload.host_key, host().public_hex());
    assert_eq!(payload.addresses, vec!["192.168.9.155:7777"]);
    let text = payload.to_qr_text();
    assert!(text.starts_with("purlis-pair:"), "{text}");
    assert_eq!(Payload::from_qr_text(&text).unwrap(), payload);
    let (code, ends) = desk.window(now).unwrap();
    assert_eq!(code, payload.code);
    assert_eq!(ends, now + WINDOW);
}

#[test]
fn a_window_ends_after_five_minutes_and_a_new_one_replaces_the_old_code() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    let now = Instant::now();
    let first = desk.open_window(&host(), HOST, vec![], now);
    assert!(desk.window(now + WINDOW).is_none(), "ended");
    let second = desk.open_window(&host(), HOST, vec![], now);
    assert_ne!(first.code, second.code);
    assert_eq!(desk.window(now).unwrap().0, second.code);
}

#[test]
fn a_spent_code_is_gone_and_a_closed_window_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    let now = Instant::now();
    desk.open_window(&host(), HOST, vec![], now);
    assert!(desk.take(now).is_some(), "the code, taken once");
    assert!(desk.take(now).is_none(), "and never twice");
    assert!(desk.window(now).is_none());
    desk.open_window(&host(), HOST, vec![], now);
    desk.close_window();
    assert!(desk.window(now).is_none());
    assert!(desk.take(now).is_none());
}

#[test]
fn a_confirmed_offer_pins_the_device_and_answers_paired() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    desk.open_window(&host(), HOST, vec![], Instant::now());
    let ticket = desk.offer(offer(1), soon());
    let (shown, what) = desk.pending().unwrap();
    assert_eq!(shown, ticket);
    assert_eq!(
        what.fingerprint,
        purlis_core::remote::fingerprint(&[1u8; 32])
    );
    assert_eq!(
        desk.confirm(ticket, 1_760_000_000).unwrap(),
        Verdict::Paired {
            device: HOST.into()
        }
    );
    assert_eq!(
        desk.take_verdict(ticket),
        Some(Verdict::Paired {
            device: HOST.into()
        })
    );
    let pinned = peers::read(dir.path()).unwrap();
    assert_eq!(pinned.len(), 1);
    assert_eq!(pinned[0].name, "Noro's iPhone");
    assert_eq!(pinned[0].key, [1u8; 32]);
    assert_eq!(pinned[0].device, PHONE);
    assert!(desk.pending().is_none());
}

#[test]
fn a_cancelled_offer_pins_nothing_and_answers_refused() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    let ticket = desk.offer(offer(1), soon());
    assert!(matches!(desk.cancel(ticket), Verdict::Refused { .. }));
    assert!(matches!(
        desk.take_verdict(ticket),
        Some(Verdict::Refused { .. })
    ));
    assert_eq!(peers::read(dir.path()).unwrap(), vec![]);
}

#[test]
fn an_unconfirmed_pairing_pins_nothing_and_spends_the_code() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    let now = Instant::now();
    desk.open_window(&host(), HOST, vec![], now);
    assert!(desk.take(now).is_some());
    let ticket = desk.offer(offer(1), soon());
    // Nobody answers: waiting until the deadline is a refusal, and nothing is pinned.
    let verdict = desk.wait_verdict(ticket, Instant::now() + Duration::from_millis(50));
    assert!(matches!(verdict, Verdict::Refused { .. }), "{verdict:?}");
    assert_eq!(peers::read(dir.path()).unwrap(), vec![]);
    assert!(desk.window(now).is_none(), "the code was spent");
    assert!(desk.pending().is_none(), "the offer left the desk");
}

#[test]
fn waiting_for_a_verdict_wakes_when_the_window_answers() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Arc::new(Desk::new(dir.path().to_path_buf()));
    desk.open_window(&host(), HOST, vec![], Instant::now());
    let ticket = desk.offer(offer(1), soon());
    let waiter = {
        let desk = Arc::clone(&desk);
        std::thread::spawn(move || {
            desk.wait_verdict(ticket, Instant::now() + Duration::from_secs(5))
        })
    };
    std::thread::sleep(Duration::from_millis(20));
    desk.confirm(ticket, 1_760_000_000).unwrap();
    assert_eq!(
        waiter.join().unwrap(),
        Verdict::Paired {
            device: HOST.into()
        }
    );
}

#[test]
fn unpairing_removes_the_record_and_bumps_the_generation() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    desk.open_window(&host(), HOST, vec![], Instant::now());
    let ticket = desk.offer(offer(1), soon());
    desk.confirm(ticket, 1_760_000_000).unwrap();
    let seen = Arc::new(Mutex::new(vec![]));
    let sink = Arc::clone(&seen);
    desk.on_revoke(Box::new(move |generation| {
        sink.lock().unwrap().push(generation)
    }));
    let before = desk.generation();
    let removed = desk.unpair(PHONE).unwrap().unwrap();
    assert_eq!(removed.key, [1u8; 32]);
    assert_eq!(desk.generation(), before + 1);
    assert_eq!(*seen.lock().unwrap(), vec![before + 1]);
    assert_eq!(peers::read(dir.path()).unwrap(), vec![]);
    assert!(desk.unpair(PHONE).unwrap().is_none());
}

#[test]
fn a_withdrawn_offer_cannot_be_confirmed_and_pins_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    desk.open_window(&host(), HOST, vec![], Instant::now());
    let ticket = desk.offer(offer(1), soon());
    // The gateway gave the link up (a second frame, the phone went away): the offer leaves
    // the desk, and a Pair pressed late pins nothing.
    desk.withdraw(ticket);
    assert!(desk.pending().is_none());
    assert!(desk.confirm(ticket, 1_760_000_000).is_err());
    assert_eq!(peers::read(dir.path()).unwrap(), vec![]);
}

#[test]
fn an_offer_past_its_window_cannot_be_confirmed() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    desk.open_window(&host(), HOST, vec![], Instant::now());
    let ticket = desk.offer(offer(1), Instant::now() - Duration::from_secs(1));
    assert!(desk.confirm(ticket, 1_760_000_000).is_err());
    assert_eq!(peers::read(dir.path()).unwrap(), vec![]);
    assert!(
        desk.pending().is_none(),
        "an offer past its window is not shown"
    );
}

#[test]
fn a_confirmation_in_progress_outlasts_the_waiters_deadline() {
    // The window pressed Pair, and the record is being written while the gateway's wait
    // reaches its deadline: the wait keeps waiting for the answer in progress, so the host
    // never pins a key while the phone is told "refused".
    let dir = tempfile::tempdir().unwrap();
    let desk = Arc::new(Desk::new(dir.path().to_path_buf()));
    desk.open_window(&host(), HOST, vec![], Instant::now());
    let ticket = desk.offer(offer(1), soon());
    let confirming = desk.begin_confirm(ticket).unwrap();
    let waiter = {
        let desk = Arc::clone(&desk);
        std::thread::spawn(move || desk.wait_verdict(ticket, Instant::now()))
    };
    std::thread::sleep(Duration::from_millis(50));
    assert!(
        !waiter.is_finished(),
        "the wait holds for the answer in progress"
    );
    desk.finish_confirm(confirming, 1_760_000_000).unwrap();
    assert_eq!(
        waiter.join().unwrap(),
        Verdict::Paired {
            device: HOST.into()
        }
    );
    assert_eq!(peers::read(dir.path()).unwrap().len(), 1);
}

#[test]
fn a_confirmation_that_cannot_write_the_record_answers_refused_and_pins_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    desk.open_window(&host(), HOST, vec![], Instant::now());
    // A key already pinned under another device: `peers::add` refuses, so the confirmation
    // fails and the phone hears refused, not paired.
    peers::add(
        dir.path(),
        peers::Peer {
            device: "01J9OTHER00000000000000000".into(),
            name: "another phone".into(),
            key: [1u8; 32],
            paired: 1,
        },
    )
    .unwrap();
    let ticket = desk.offer(offer(1), soon());
    assert!(desk.confirm(ticket, 1_760_000_000).is_err());
    assert!(matches!(
        desk.take_verdict(ticket),
        Some(Verdict::Refused { .. })
    ));
    assert_eq!(peers::read(dir.path()).unwrap().len(), 1);
}

#[test]
fn a_confirmation_of_an_offer_that_is_not_pending_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    assert!(desk.confirm(Ticket(99), 1).is_err());
    assert_eq!(peers::read(dir.path()).unwrap(), vec![]);
}
