// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::forge::recorded::Recorded;
use crate::secrets::keyring;

/// The recording `case` of GitHub's signing keys, `signer`'s key put in for `@KEY@`.
fn recording(case: &str, signer: &Signer) -> Recorded {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/forge_contract/signing_keys/github")
        .join(format!("{case}.json"));
    let text = std::fs::read_to_string(&file)
        .unwrap()
        .replace("@KEY@", &signer.public_line());
    Recorded::parse(&text).unwrap()
}

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
        answered_over(&none, &forge(), &on_account(&signer)).unwrap(),
        Heard::KeyOnAccount {
            on: false,
            other_devices: 0
        }
    );

    let added = recording("added_then_listed", &signer);
    signingkeys::add_over(&added, &forge(), &signer, "purlis · studio").unwrap();
    assert_eq!(
        answered_over(&added, &forge(), &on_account(&signer)).unwrap(),
        Heard::KeyOnAccount {
            on: true,
            other_devices: 1
        }
    );
    assert!(added.unspent().is_empty());

    let verified = recording("commit_verified", &signer);
    let asked = Asked::Check {
        repo: "octo/hello".to_owned(),
        commit: "0123abc".to_owned(),
    };
    let Heard::Checked(checked) = answered_over(&verified, &forge(), &asked).unwrap() else {
        panic!("a check is answered with what the forge said of the commit");
    };
    assert!(checked.verified);
}

#[test]
fn a_sign_in_that_may_not_read_signing_keys_is_heard_as_needing_permission() {
    let signer = Signer::generate().unwrap();
    let recorded = recording("list_needs_permission", &signer);
    assert_eq!(
        answered_over(&recorded, &forge(), &on_account(&signer)).unwrap(),
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
            answered_over(&recorded, &forge(), &asked).is_err(),
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
    assert!(crate::commitsign::forget(config.path(), &store).unwrap());
    assert_eq!(
        crate::commitsign::kept(config.path(), &store).unwrap(),
        None
    );
    assert!(!crate::commitsign::forget(config.path(), &store).unwrap());
}

#[cfg(unix)]
mod over_the_window_line {
    use std::time::Duration;

    use super::*;
    use crate::windowline::{self, Answered, Said};

    /// The command asks over the line; the window answers with its own transport (here a
    /// recording); the command hears what the forge said, and holds no sign-in of its own.
    #[test]
    fn the_command_hears_what_the_window_asked_the_forge() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("purlis").join("window.sock");
        let signer = Signer::generate().unwrap();
        let line = signer.public_line();
        let recorded = std::sync::Arc::new(recording("added_then_listed", &signer));
        let theirs = std::sync::Arc::clone(&recorded);
        windowline::serve_on(&socket, move |asked| match asked {
            windowline::Asked::Signing(asked) => {
                match answered_over(theirs.as_ref(), &forge(), &asked) {
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
        signingkeys::add_over(recorded.as_ref(), &forge(), &signer, "purlis · studio").unwrap();

        let within = Duration::from_secs(10);
        let reached = reach(Asked::KeyOnAccount { key: line }, |asked| {
            windowline::ask_on(&socket, asked, within)
        });
        assert_eq!(
            reached,
            Reached::Heard(Heard::KeyOnAccount {
                on: true,
                other_devices: 1
            })
        );
    }

    #[test]
    fn no_window_is_said_as_none_and_an_old_windows_answer_as_no_answer() {
        let asked = || Asked::Check {
            repo: "octo/hello".to_owned(),
            commit: "0123abc".to_owned(),
        };
        assert_eq!(reach(asked(), |_| Answered::NoWindow), Reached::NoWindow);
        assert_eq!(
            reach(asked(), |_| Answered::Said(Said::Asked { note: None })),
            Reached::NoAnswer
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
