// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::sshroute::HostKey;

const KEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINiGzfjDDJsgYLYCmdkf8Fel9sE6Ma14Y2+c33Y/qZ2n";
const OTHER: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExxHsYI9UnIswq1HBUE9zECyyjh0ixyp2O+1SfSzZmy";

fn key(text: &str) -> HostKey {
    HostKey::parse(text).unwrap()
}

fn key_changed() -> Finding {
    Finding::KeyChanged {
        host: "build-box".into(),
        pinned: key(KEY),
        now: key(OTHER),
    }
}

fn ca_changed() -> Finding {
    Finding::CaChanged {
        host: "build-box".into(),
        pinned: "AA:BB".into(),
        now: "CC:DD".into(),
        file: "/home/me/.docker/ca.pem".into(),
    }
}

fn engine_changed(podman: bool) -> Finding {
    Finding::EngineChanged {
        host: "build-box".into(),
        pinned: "OLD:ENGINE".into(),
        now: "NEW:ENGINE".into(),
        podman,
    }
}

#[test]
fn a_changed_key_names_the_pool_the_key_both_fingerprints_and_the_usual_reason() {
    let change = of("builds", &key_changed()).expect("a pin change");
    assert_eq!(change.what, Changed::Key);
    assert_eq!(change.pinned, key(KEY).fingerprint());
    assert_eq!(change.now, key(OTHER).fingerprint());
    assert_eq!(change.title, "Pool builds: build-box shows a different key");
    let said = said(&change);
    assert!(said.contains("Pool builds"), "{said}");
    assert!(said.contains(&key(KEY).fingerprint()), "{said}");
    assert!(said.contains(&key(OTHER).fingerprint()), "{said}");
    assert!(said.contains("reinstalled"), "{said}");
    assert!(
        said.contains("starts no runner, opens no new link and sends no vault value"),
        "{said}"
    );
    assert!(said.contains("Links already open stay open"), "{said}");
}

#[test]
fn a_changed_ca_names_its_file_both_fingerprints_and_the_usual_reason() {
    let change = of("builds", &ca_changed()).expect("a pin change");
    assert_eq!(change.what, Changed::Ca);
    assert_eq!(
        (change.pinned.as_str(), change.now.as_str()),
        ("AA:BB", "CC:DD")
    );
    assert_eq!(change.title, "Pool builds: build-box's CA changed");
    let said = said(&change);
    assert!(said.contains("/home/me/.docker/ca.pem"), "{said}");
    assert!(said.contains("AA:BB") && said.contains("CC:DD"), "{said}");
    assert!(said.contains("the CA is replaced"), "{said}");
    assert!(
        said.contains("starts no runner, opens no new link and sends no vault value"),
        "{said}"
    );
}

#[test]
fn a_changed_engine_says_its_runners_are_on_another_engine_until_it_is_trusted() {
    for podman in [false, true] {
        let change = of("builds", &engine_changed(podman)).expect("a pin change");
        assert_eq!(change.what, Changed::Engine);
        let said = said(&change);
        assert!(said.contains("OLD:ENGINE") && said.contains("NEW:ENGINE"), "{said}");
        assert!(said.contains("reset or reinstalled"), "{said}");
        assert!(said.contains("on another engine"), "{said}");
        assert!(!said.contains("removed"), "never said to be removed: {said}");
        assert!(
            said.contains("Once you trust this one, you can make the runner again"),
            "{said}"
        );
    }
    assert_eq!(
        of("builds", &engine_changed(false)).unwrap().title,
        "Pool builds: Docker on build-box isn't the one you trusted"
    );
    assert_eq!(
        of("builds", &engine_changed(true)).unwrap().title,
        "Pool builds: Podman on build-box isn't the one you trusted"
    );
}

#[test]
fn only_a_changed_key_ca_or_engine_is_a_pin_change() {
    for finding in [
        Finding::NoAnswer {
            host: "build-box".into(),
            port: 22,
        },
        Finding::SignInRefused {
            host: "build-box".into(),
        },
        Finding::Here("no ssh".into()),
    ] {
        assert_eq!(of("builds", &finding), None, "{finding:?}");
    }
}

#[test]
fn a_change_found_is_kept_until_the_pin_holds_again_and_the_window_is_told_each_time() {
    let told = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = told.clone();
    let changes = Changes::default().telling(Box::new(move || {
        counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));
    let change = of("builds", &key_changed()).unwrap();
    changes.found(change.clone());
    changes.found(change.clone());
    assert_eq!(changes.all(), vec![change.clone()]);
    assert_eq!(
        told.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the same change is told once"
    );
    changes.holds("other");
    assert_eq!(told.load(std::sync::atomic::Ordering::SeqCst), 1);
    changes.holds("builds");
    assert_eq!(changes.all(), Vec::<PinChange>::new());
    assert_eq!(told.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[test]
fn doctor_says_a_pin_change_in_the_notices_words_and_exits_1() {
    let (said, code) = doctor_said("builds", Err(&key_changed()));
    assert_eq!(code, 1);
    let change = of("builds", &key_changed()).unwrap();
    assert!(said.starts_with(&change.title), "{said}");
    assert!(said.contains(&self::said(&change)), "{said}");
    assert!(said.contains("Check and trust again"), "{said}");
    assert!(said.contains("Remove pool"), "{said}");
    for finding in [ca_changed(), engine_changed(false)] {
        let (said, code) = doctor_said("builds", Err(&finding));
        assert_eq!(code, 1);
        assert!(
            said.contains(&self::said(&of("builds", &finding).unwrap())),
            "{said}"
        );
    }
}

#[test]
fn doctor_says_another_finding_in_pool_setups_words_and_a_pin_that_holds_exits_0() {
    let refused = Finding::SignInRefused {
        host: "build-box".into(),
    };
    let (said, code) = doctor_said("builds", Err(&refused));
    assert_eq!(code, 1);
    assert!(said.contains(&crate::poolsetup::connect_said(&refused)), "{said}");
    let (said, code) = doctor_said("builds", Ok("build-box"));
    assert_eq!(code, 0);
    assert_eq!(
        said,
        "Pool builds: build-box is the machine you trusted, and its engine the one you trusted.\n"
    );
}
