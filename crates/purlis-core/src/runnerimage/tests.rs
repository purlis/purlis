// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use base64::Engine as _;

use super::*;

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

struct Key(minisign::KeyPair);

impl Key {
    fn new() -> Key {
        Key(minisign::KeyPair::generate_unencrypted_keypair().unwrap())
    }

    fn public(&self) -> String {
        b64(self.0.pk.to_box().unwrap().into_string().as_bytes())
    }

    /// A signature of `line` as the release makes one: the updater's key, `version` in its
    /// trusted comment.
    fn sign(&self, line: &str, version: &str) -> String {
        let trusted = format!("timestamp:1790077121\tfile:runner-image\tversion:{version}");
        let signature = minisign::sign(
            None,
            &self.0.sk,
            std::io::Cursor::new(line.as_bytes()),
            Some(&trusted),
            Some("signature from tauri secret key"),
        )
        .unwrap();
        b64(signature.into_string().as_bytes())
    }
}

fn manifest(version: &str, image: &str, signature: &str) -> serde_json::Value {
    serde_json::json!({
        "version": version,
        "platforms": {},
        "runnerImage": { "image": image, "signature": signature },
    })
}

#[test]
fn the_runner_image_the_updater_key_signed_for_this_version_is_taken() {
    let key = Key::new();
    let found = of_this_version(
        &manifest("0.4.2-dev.17", IMAGE, &key.sign(IMAGE, "0.4.2-dev.17")),
        &key.public(),
        "0.4.2-dev.17",
    )
    .unwrap();
    assert_eq!(found.unwrap().as_str(), IMAGE);
}

#[test]
fn a_manifest_of_another_version_or_with_no_runner_image_names_none() {
    let key = Key::new();
    let signed = key.sign(IMAGE, "0.4.2-dev.18");
    assert_eq!(
        of_this_version(
            &manifest("0.4.2-dev.18", IMAGE, &signed),
            &key.public(),
            "0.4.2-dev.17"
        ),
        Ok(None)
    );
    assert_eq!(
        of_this_version(
            &serde_json::json!({ "version": "0.4.2-dev.17" }),
            &key.public(),
            "0.4.2-dev.17"
        ),
        Ok(None)
    );
}

#[test]
fn a_runner_image_not_signed_by_the_updater_key_for_this_version_over_its_line_is_refused() {
    let (key, other) = (Key::new(), Key::new());
    let version = "0.4.2-dev.17";
    let another = "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    for (named, signature) in [
        // Another key's.
        (IMAGE, other.sign(IMAGE, version)),
        // Another line's: the image swapped for another under the same signature.
        (another, key.sign(IMAGE, version)),
        // Another version's.
        (IMAGE, key.sign(IMAGE, "0.4.2-dev.16")),
        // No signature.
        (IMAGE, String::new()),
    ] {
        assert!(
            of_this_version(
                &manifest(version, named, &signature),
                &key.public(),
                version
            )
            .is_err(),
            "{named} {signature}"
        );
    }
}

#[test]
fn a_signed_image_that_is_not_purlis_own_or_not_pinned_is_refused() {
    let key = Key::new();
    let version = "0.4.2";
    for line in [
        "registry.example/team/runner@sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
        "ghcr.io/purlis/purlis-runner:main",
    ] {
        assert!(
            of_this_version(
                &manifest(version, line, &key.sign(line, version)),
                &key.public(),
                version
            )
            .is_err(),
            "{line}"
        );
    }
}

/// **A new pool is offered this version's image, or a newer version's from the channel's newest
/// manifest** (the person's ruling of 2026-10-30, round 2): a machine behind its channel, as a
/// dev machine is after every merge, is still offered a verified image, said to be that
/// version's.
#[test]
fn a_new_pool_is_offered_this_versions_image_or_the_channels_newer_one() {
    let key = Key::new();
    let ours = "0.4.2-dev.17";
    assert_eq!(
        offered(
            &manifest(ours, IMAGE, &key.sign(IMAGE, ours)),
            &key.public(),
            ours
        ),
        Ok(Offered::Ours(Image::pinned(IMAGE).unwrap()))
    );
    let newer = "0.4.2-dev.18";
    assert_eq!(
        offered(
            &manifest(newer, IMAGE, &key.sign(IMAGE, newer)),
            &key.public(),
            ours
        ),
        Ok(Offered::Newer {
            image: Image::pinned(IMAGE).unwrap(),
            version: newer.to_owned()
        })
    );
}

/// **What a manifest offers is held as strictly as this version's image is**: one signed by
/// another key, for another version than the manifest's, or over another line is refused.
#[test]
fn a_newer_versions_image_is_checked_as_strictly_as_this_versions() {
    let (key, other) = (Key::new(), Key::new());
    let (ours, newer) = ("0.4.2", "0.4.3");
    for signature in [
        other.sign(IMAGE, newer),
        key.sign(IMAGE, ours),
        key.sign("ghcr.io/purlis/purlis-runner:main", newer),
        String::new(),
    ] {
        assert!(
            offered(&manifest(newer, IMAGE, &signature), &key.public(), ours).is_err(),
            "{signature}"
        );
    }
}

/// **None is offered from a manifest that names none, or is of an older version**; one that
/// names no version is refused.
#[test]
fn none_is_offered_from_a_manifest_with_no_image_or_of_an_older_version() {
    let key = Key::new();
    let ours = "0.4.2";
    for version in [ours, "0.4.3"] {
        assert_eq!(
            offered(
                &serde_json::json!({ "version": version }),
                &key.public(),
                ours
            ),
            Ok(Offered::None)
        );
    }
    let older = "0.4.1";
    assert_eq!(
        offered(
            &manifest(older, IMAGE, &key.sign(IMAGE, older)),
            &key.public(),
            ours
        ),
        Ok(Offered::None)
    );
    assert!(offered(&serde_json::json!({}), &key.public(), ours).is_err());
}
