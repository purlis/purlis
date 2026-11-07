// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::forge::recorded::Recorded;
use crate::forge::signingkeys::recording;
use crate::secrets::keyring;

fn on_account(signer: &Signer) -> Asked {
    Asked::KeyOnAccount {
        key: signer.public_line(),
    }
}

/// No key, then made (in the fenced file keychain), then not on the account, then added by
/// the window's sign-in and listed, then a pushed commit Verified: the whole of setup that does
/// not wait on the person, as the forge answers it.
#[test]
fn no_key_then_made_then_added_then_listed_then_verified() {
    let config = tempfile::tempdir().unwrap();
    let keys = tempfile::tempdir().unwrap();
    crate::machine::device_id(config.path()).unwrap();
    let store = keyring::FileStore::at(keys.path().join("keyring.json"));

    assert_eq!(
        crate::commitsign::kept(config.path(), &store).unwrap(),
        None
    );
    let (signer, had) = key_made(config.path(), &store).unwrap();
    assert!(!had);
    assert_eq!(
        crate::commitsign::kept(config.path(), &store).unwrap(),
        Some(signer.clone())
    );
    let (again, had) = key_made(config.path(), &store).unwrap();
    assert!(had);
    assert_eq!(again, signer, "a second setup keeps the key it made");

    let none = recording("none_listed", &signer);
    assert_eq!(
        answered_over(&none, &github(), &on_account(&signer)).unwrap(),
        Heard::KeyOnAccount { on: false }
    );

    let added = recording("added_then_listed", &signer);
    signingkeys::add_over(&added, &github(), &signer, "purlis · studio").unwrap();
    assert_eq!(
        answered_over(&added, &github(), &on_account(&signer)).unwrap(),
        Heard::KeyOnAccount { on: true }
    );
    assert!(added.unspent().is_empty());

    let verified = recording("commit_verified", &signer);
    let asked = Asked::Check {
        repo: "octo/hello".to_owned(),
        commit: "0123abc".to_owned(),
    };
    let Heard::Checked(checked) = answered_over(&verified, &github(), &asked).unwrap() else {
        panic!("a check is answered with what the forge said of the commit");
    };
    assert!(checked.verified);
}

#[test]
fn a_sign_in_that_may_not_read_signing_keys_is_heard_as_needing_permission() {
    let signer = Signer::generate().unwrap();
    let recorded = recording("list_needs_permission", &signer);
    assert_eq!(
        answered_over(&recorded, &github(), &on_account(&signer)).unwrap(),
        Heard::NeedsPermission
    );
}

#[test]
fn what_is_not_one_public_key_is_refused_before_anything_is_sent() {
    let recorded = Recorded::parse(r#"{"source": "nothing", "exchanges": []}"#).unwrap();
    for key in [
        "",
        "ssh-ed25519",
        "not a key at all",
        "ssh-ed25519 AAAA\nssh-rsa BBBB",
    ] {
        let asked = Asked::KeyOnAccount {
            key: key.to_owned(),
        };
        assert!(
            answered_over(&recorded, &github(), &asked).is_err(),
            "{key:?}"
        );
    }
}

#[test]
fn the_key_is_removed_from_the_keychain() {
    let config = tempfile::tempdir().unwrap();
    let keys = tempfile::tempdir().unwrap();
    crate::machine::device_id(config.path()).unwrap();
    let store = keyring::FileStore::at(keys.path().join("keyring.json"));
    key_made(config.path(), &store).unwrap();
    assert!(key_removed(config.path(), &store).unwrap());
    assert_eq!(
        crate::commitsign::kept(config.path(), &store).unwrap(),
        None
    );
    assert!(!key_removed(config.path(), &store).unwrap());
}

#[cfg(unix)]
mod over_the_window_line {
    use std::time::Duration;

    use super::*;
    use crate::windowline::{self, Answered, Said};

    /// The command asks over the line; the window answers with its own transport (here a
    /// recording); the command hears what the forge said, and holds no sign-in of its own.
    #[test]
    fn the_command_hears_what_the_window_asked_github() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("purlis").join("window.sock");
        let signer = Signer::generate().unwrap();
        let line = signer.public_line();
        let recorded = std::sync::Arc::new(recording("added_then_listed", &signer));
        let theirs = std::sync::Arc::clone(&recorded);
        windowline::serve_on(&socket, move |asked| match asked {
            windowline::Asked::Signing(asked) => {
                match answered_over(theirs.as_ref(), &github(), &asked) {
                    Ok(heard) => Said::Signing(heard),
                    Err(why) => Said::Refused { why },
                }
            }
            _ => Said::Refused {
                why: "not this test's".to_owned(),
            },
        })
        .unwrap();
        // The add is the window's alone, so it is spent here before the command asks.
        signingkeys::add_over(recorded.as_ref(), &github(), &signer, "purlis · studio").unwrap();

        let within = Duration::from_secs(10);
        let reached = reach(Asked::KeyOnAccount { key: line }, |asked| {
            windowline::ask_on(&socket, asked, within)
        });
        assert_eq!(reached, Reached::Heard(Heard::KeyOnAccount { on: true }));
    }

    #[test]
    fn no_window_no_answer_and_another_questions_answer_are_told_apart() {
        let asked = || Asked::Check {
            repo: "octo/hello".to_owned(),
            commit: "0123abc".to_owned(),
        };
        assert_eq!(reach(asked(), |_| Answered::NoWindow), Reached::NoWindow);
        assert_eq!(reach(asked(), |_| Answered::NoAnswer), Reached::NoAnswer);
        assert_eq!(
            reach(asked(), |_| Answered::Said(Said::Asked { note: None })),
            Reached::OtherAnswer
        );
        assert_eq!(
            reach(asked(), |_| Answered::Said(Said::Refused {
                why: "not signed in".to_owned()
            })),
            Reached::Refused("not signed in".to_owned())
        );
    }

    /// The profile lines' answer never takes a signing ask for a profile's.
    #[test]
    fn a_signing_ask_is_no_profiles() {
        let dir = tempfile::tempdir().unwrap();
        let said = windowline::answer(
            dir.path(),
            windowline::Asked::Signing(Asked::KeyOnAccount { key: "k".into() }),
            |_| panic!("no forget"),
            |_, _| panic!("no keep"),
            |_, _, _| panic!("no approve"),
            |_, _, _| panic!("no take-off"),
        );
        assert!(matches!(said, Said::Refused { .. }));
    }
}
