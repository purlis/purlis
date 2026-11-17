// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const LINE: &str = "ghcr.io/purlis/purlis-runner@sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

/// A keypair as `tauri signer generate` makes one, and the app's and the release's spelling of
/// each half.
fn a_key(password: Option<&str>) -> (String, String) {
    let pair = match password {
        Some(password) => minisign::KeyPair::generate_encrypted_keypair(Some(password.to_owned())),
        None => minisign::KeyPair::generate_unencrypted_keypair(),
    }
    .unwrap();
    let secret = pair.sk.to_box(None).unwrap().into_string();
    let public = pair.pk.to_box().unwrap().into_string();
    (
        base64_text::encode(secret.as_bytes()),
        base64_text::encode(public.as_bytes()),
    )
}

#[test]
fn a_line_signed_with_the_updaters_key_is_one_the_app_takes_for_that_version() {
    for password in [None, Some("hunter2")] {
        let (secret, public) = a_key(password);
        let signature = signed(LINE, &secret, password, "0.4.2-dev.17", 1_790_077_121).unwrap();
        let manifest = serde_json::json!({
            "version": "0.4.2-dev.17",
            "runnerImage": { "image": LINE, "signature": signature },
        });
        let found =
            purlis_core::runnerimage::of_this_version(&manifest, &public, "0.4.2-dev.17").unwrap();
        // Which case failed is said by whether the key has a password, never by the password.
        let sealed = password.is_some();
        assert_eq!(
            found.unwrap().as_str(),
            LINE,
            "a key with a password: {sealed}"
        );
    }
}

#[test]
fn a_key_the_password_does_not_open_signs_nothing() {
    let (secret, _) = a_key(Some("hunter2"));
    assert!(signed(LINE, &secret, Some("wrong"), "0.4.2", 1).is_err());
    assert!(signed(LINE, "not base64!", None, "0.4.2", 1).is_err());
}

#[test]
fn a_runner_binary_signed_with_the_updaters_key_is_one_the_desktop_installs_for_that_version_and_platform()
 {
    use purlis_core::runnerinstall::{Target, verify};
    let bytes = b"\x7fELF the purlis of a runner";
    for password in [None, Some("hunter2")] {
        let (secret, public) = a_key(password);
        for target in Target::ALL {
            let signature =
                signed_runner(bytes, target, &secret, password, "1.4.0", 1_790_077_121).unwrap();
            let verified = verify(&public, bytes, &signature, "1.4.0", target)
                .expect("the desktop takes what the release signed");
            assert_eq!(
                verified.digest(),
                purlis_core::runnerinstall::digest_of(bytes)
            );
            // Never as another platform's asset, nor another version's.
            let other = Target::ALL.into_iter().find(|it| *it != target).unwrap();
            assert!(verify(&public, bytes, &signature, "1.4.0", other).is_err());
            assert!(verify(&public, bytes, &signature, "1.4.1", target).is_err());
        }
    }
}

#[test]
fn a_runner_binary_is_signed_only_under_a_version_it_can_be_published_under() {
    let (secret, _) = a_key(None);
    let target = purlis_core::runnerinstall::Target::LinuxX86_64;
    assert!(signed_runner(b"x", target, &secret, None, "../1.4.0", 1).is_err());
    assert!(signed_runner(b"x", target, "not base64!", None, "1.4.0", 1).is_err());
}
