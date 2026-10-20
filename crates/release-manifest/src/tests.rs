// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// Base64, for building the fixtures the decoder is then pointed at.
fn encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 0x3f) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A keypair made for this run and dropped with it, and the `pubkey` string Tauri would be
/// configured with for it: base64 over the whole `.pub` file, as `tauri signer generate`
/// writes it.
struct Signer {
    keys: minisign::KeyPair,
}

impl Signer {
    fn new() -> Self {
        Self {
            keys: minisign::KeyPair::generate_unencrypted_keypair().unwrap(),
        }
    }

    fn pubkey(&self) -> String {
        encode(self.keys.pk.to_box().unwrap().into_string().as_bytes())
    }

    /// A `.sig` exactly as `@tauri-apps/cli` 2.11.5 writes one: the same untrusted comment,
    /// the same trusted-comment layout (`timestamp`, `file`, then `version` when given),
    /// and the whole signature file base64'd.
    fn sign(&self, artifact: &[u8], version: Option<&str>) -> String {
        let mut trusted = "timestamp:1790077121\tfile:charter.app.tar.gz".to_owned();
        if let Some(version) = version {
            trusted.push_str("\tversion:");
            trusted.push_str(version);
        }
        let sig = minisign::sign(
            None,
            &self.keys.sk,
            std::io::Cursor::new(artifact),
            Some(&trusted),
            Some("signature from tauri secret key"),
        )
        .unwrap();
        encode(sig.into_string().as_bytes())
    }

    fn entries(&self, version: &str) -> Vec<Entry> {
        TARGETS
            .iter()
            .map(|target| {
                let artifact = format!("the {target} bundle of {version}").into_bytes();
                Entry {
                    target: (*target).to_owned(),
                    url: format!(
                        "https://github.com/purlis/purlis/releases/download/v{version}/{target}"
                    ),
                    signature: self.sign(&artifact, Some(version)),
                    artifact,
                }
            })
            .collect()
    }

    fn assemble(
        &self,
        version: &str,
        rows: &[Entry],
    ) -> Result<(&'static str, String), Refused> {
        assemble(Channel::Stable, &self.pubkey(), version, "x", "", rows)
    }
}

#[test]
fn a_whole_release_becomes_the_manifest_tauri_reads() {
    let signer = Signer::new();
    let (name, text) = assemble(
        Channel::Stable,
        &signer.pubkey(),
        "0.2.0",
        "2026-09-22T10:00:00Z",
        "notes",
        &signer.entries("0.2.0"),
    )
    .expect("a complete, signed release");
    assert_eq!(name, "latest.json");
    let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(doc["version"], "0.2.0");
    assert_eq!(doc["pub_date"], "2026-09-22T10:00:00Z");
    for target in TARGETS {
        assert!(
            doc["platforms"][target]["url"].is_string(),
            "{target} has no url"
        );
        assert!(
            !doc["platforms"][target]["signature"]
                .as_str()
                .unwrap()
                .is_empty()
        );
    }
    assert!(text.ends_with('\n'), "a published file ends in a newline");
}

#[test]
fn the_two_channels_write_two_files_and_never_one() {
    let signer = Signer::new();
    let rows = signer.entries("0.2.0");
    let dev = assemble(Channel::Dev, &signer.pubkey(), "0.2.0", "x", "", &rows).unwrap();
    let stable = assemble(Channel::Stable, &signer.pubkey(), "0.2.0", "x", "", &rows).unwrap();
    assert_eq!(dev.0, "dev.json");
    assert_eq!(stable.0, "latest.json");
    // Same release, two names, identical bodies: the channel is which FILE it lands in.
    assert_eq!(dev.1, stable.1);
}

#[test]
fn every_channel_publishes_its_weekly_manifest_as_the_manifest_byte_for_byte() {
    let signer = Signer::new();
    let rows = signer.entries("0.2.0");
    for channel in Channel::ALL {
        let (name, text) =
            assemble(channel, &signer.pubkey(), "0.2.0", "x", "", &rows).unwrap();
        let files = published(channel, &text);
        let names: Vec<&str> = files.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, [name, channel.weekly_manifest()]);
        // Identical bytes: a machine reading either is updated the same way, and the
        // weekly one's download count is the only thing that tells them apart (OB-17).
        assert!(files.iter().all(|(_, body)| *body == text));
    }
}

#[test]
fn an_unsigned_artifact_never_reaches_a_manifest() {
    // The whole reason this crate exists. Tauri's bundler skips signing in silence when
    // no key is set, so "" is exactly what a release with no TAURI_SIGNING_PRIVATE_KEY
    // hands the manifest builder.
    let signer = Signer::new();
    for (sig, expected) in [
        ("", "never signed"),
        ("   \n", "never signed"),
        ("not base64 at all !!", "not base64"),
        // Base64, decodes, is text — and is not a signature.
        ("aGVsbG8gdGhlcmU=", "untrusted comment"),
    ] {
        let mut rows = signer.entries("0.2.0");
        rows[0].signature = sig.to_owned();
        let refused = signer
            .assemble("0.2.0", &rows)
            .expect_err("an unsigned artifact was published");
        let Refused::NotASignature { target, why } = &refused else {
            panic!("{sig:?} was refused as {refused:?}, not as a missing signature");
        };
        assert_eq!(target, TARGETS[0]);
        assert!(why.contains(expected), "{why}");
    }
}

#[test]
fn a_signature_by_another_key_is_refused_though_it_is_perfectly_formed() {
    // The rotated-secret case: the release was signed, the signature is well formed, the
    // version matches — and it was made by a key the app does not carry. Every app on the
    // channel would refuse it, so the release must not exist.
    let ours = Signer::new();
    let theirs = Signer::new();
    let mut rows = ours.entries("0.2.0");
    rows[1].signature = theirs.sign(&rows[1].artifact, Some("0.2.0"));
    let refused = ours.assemble("0.2.0", &rows).unwrap_err();
    assert!(
        matches!(&refused, Refused::DoesNotVerify { target, .. } if target == TARGETS[1]),
        "{refused:?}"
    );
}

#[test]
fn a_signature_over_different_bytes_is_refused() {
    // A tampered or truncated artifact, or a `.sig` paired with the wrong file.
    let signer = Signer::new();
    let mut rows = signer.entries("0.2.0");
    rows[0].artifact.push(b'!');
    assert!(matches!(
        signer.assemble("0.2.0", &rows).unwrap_err(),
        Refused::DoesNotVerify { .. }
    ));
}

#[test]
fn a_trusted_comment_edited_after_signing_is_refused_before_its_version_is_believed() {
    // Take a real signature for 0.1.0 and rewrite its trusted comment to say 0.2.0. The
    // version now matches what is announced — and the global signature no longer covers
    // the comment. This is the forgery requireSignedVersion exists to stop; the check here
    // must not be fooled by the text it would otherwise trust.
    let signer = Signer::new();
    let mut rows = signer.entries("0.2.0");
    let old = signer.sign(&rows[0].artifact, Some("0.1.0"));
    let text = decode_base64_text(&old)
        .unwrap()
        .replace("version:0.1.0", "version:0.2.0");
    rows[0].signature = encode(text.as_bytes());
    assert!(matches!(
        signer.assemble("0.2.0", &rows).unwrap_err(),
        Refused::DoesNotVerify { .. }
    ));
}

#[test]
fn a_release_cannot_be_published_while_the_app_carries_the_placeholder_key() {
    let signer = Signer::new();
    let refused = assemble(
        Channel::Stable,
        purlis_core::updates::PUBKEY_UNSET,
        "0.2.0",
        "x",
        "",
        &signer.entries("0.2.0"),
    )
    .unwrap_err();
    assert!(matches!(refused, Refused::NoKey(_)), "{refused:?}");
    assert!(
        refused
            .to_string()
            .contains("generated the updater keypair")
    );
}

#[test]
fn a_signature_made_for_another_version_is_refused_by_both_numbers() {
    let signer = Signer::new();
    let mut rows = signer.entries("0.2.0");
    rows[1].signature = signer.sign(&rows[1].artifact, Some("0.1.0"));
    let refused = signer.assemble("0.2.0", &rows).unwrap_err();
    assert_eq!(
        refused,
        Refused::SignedForAnother {
            target: TARGETS[1].to_owned(),
            signed: "0.1.0".to_owned(),
            announced: "0.2.0".to_owned(),
        }
    );
    assert!(refused.to_string().contains("requireSignedVersion"));
}

#[test]
fn a_signature_from_a_cli_that_does_not_record_the_version_is_refused_by_name() {
    // `@tauri-apps/cli` 2.11.4 and older: `timestamp:…\tfile:…`, and nothing else —
    // measured on this repository's own pinned CLI before it was bumped. An app with
    // `requireSignedVersion` refuses every one of these.
    let signer = Signer::new();
    let mut rows = signer.entries("0.2.0");
    rows[0].signature = signer.sign(&rows[0].artifact, None);
    let refused = signer.assemble("0.2.0", &rows).unwrap_err();
    let Refused::NotASignature { why, .. } = &refused else {
        panic!("{refused:?}");
    };
    assert!(why.contains("2.11.5"), "{why}");
    assert!(why.contains("requireSignedVersion"), "{why}");
}

#[test]
fn a_platform_with_no_artifact_stops_the_release_rather_than_shipping_without_it() {
    let signer = Signer::new();
    for missing in TARGETS {
        let rows: Vec<Entry> = signer
            .entries("0.2.0")
            .into_iter()
            .filter(|e| e.target != missing)
            .collect();
        assert_eq!(
            signer.assemble("0.2.0", &rows).unwrap_err(),
            Refused::MissingTarget(missing)
        );
    }
    assert_eq!(
        signer.assemble("0.2.0", &[]).unwrap_err(),
        Refused::MissingTarget(TARGETS[0])
    );
}

#[test]
fn a_target_charter_does_not_publish_is_refused_rather_than_carried() {
    let signer = Signer::new();
    let mut rows = signer.entries("0.2.0");
    let artifact = b"x".to_vec();
    rows.push(Entry {
        target: "linux-x86_64".to_owned(),
        url: "https://example.invalid/x".to_owned(),
        signature: signer.sign(&artifact, Some("0.2.0")),
        artifact,
    });
    // `linux-x86_64` is the one that matters: it is the key a `.deb` install falls back
    // to, and publishing it would hand AppImage bytes to dpkg's installer.
    assert_eq!(
        signer.assemble("0.2.0", &rows).unwrap_err(),
        Refused::UnknownTarget("linux-x86_64".to_owned())
    );
}

#[test]
fn one_target_cannot_be_named_twice() {
    let signer = Signer::new();
    let mut rows = signer.entries("0.2.0");
    rows.push(rows[0].clone());
    assert_eq!(
        signer.assemble("0.2.0", &rows).unwrap_err(),
        Refused::TargetTwice(TARGETS[0].to_owned())
    );
}

#[test]
fn a_url_an_updater_will_not_fetch_is_refused_here_and_not_on_a_laptop() {
    let signer = Signer::new();
    for url in [
        "http://github.com/x",
        "ftp://x/y",
        "github.com/x",
        "",
        "HTTPS://github.com/x",
    ] {
        let mut rows = signer.entries("0.2.0");
        rows[0].url = url.to_owned();
        let refused = signer.assemble("0.2.0", &rows).unwrap_err();
        assert!(
            matches!(refused, Refused::NotHttps { .. }),
            "{url:?} was accepted: {refused:?}"
        );
    }
}

#[test]
fn a_version_tauri_cannot_parse_is_a_manifest_no_app_would_read() {
    let signer = Signer::new();
    for bad in [
        "",
        "0.2",
        "v0.2.0",
        "0.2.0.1",
        "0.02.0",
        "0.2.x",
        "0.2.0-",
        "0.2.0+",
        "latest",
        "0.2.0-dev.",
        "0.2.0 ",
    ] {
        assert_eq!(
            signer.assemble(bad, &signer.entries("0.2.0")).unwrap_err(),
            Refused::NotAVersion(bad.to_owned()),
            "{bad:?} was read as a version"
        );
    }
    for good in ["0.1.0", "0.2.0-dev.42", "1.0.0-rc.1+9d18d55", "10.20.30"] {
        assert!(is_semver(good), "{good} was refused");
    }
}

#[test]
fn the_manifest_is_byte_stable_so_a_rebuild_is_a_diff_and_not_a_shuffle() {
    let signer = Signer::new();
    let rows = signer.entries("0.2.0");
    let once = signer.assemble("0.2.0", &rows).unwrap();
    let mut backwards = rows.clone();
    backwards.reverse();
    let again = signer.assemble("0.2.0", &backwards).unwrap();
    assert_eq!(once, again, "the platform order came from the input");
}

#[test]
fn a_signature_the_real_cli_wrote_is_read_as_the_cli_meant_it() {
    // Not synthesised: the trusted comment `@tauri-apps/cli` 2.11.5 wrote for a real file
    // on 2026-09-22, re-encoded here so the parser is held to the real layout. Its key is
    // gone, so only the version reading is checked, not the signature.
    let real = "untrusted comment: signature from tauri secret key\n\
                RUQVpluvLerqpN7NeNARoWj8rsClllXwe4viKybzRlmzucaOfPi0830n4gh9OJwn7TDKQ0RFIWBe5U2ORGVjCXxTrj2MjkAYhgc=\n\
                trusted comment: timestamp:1790077121\tfile:charter_0.2.0_aarch64.app.tar.gz\tversion:0.2.0\n\
                Q8P8rBCCiQNlf52CYkoq+T+y71I2hpMFykcRSahWmedhuDlfi/vDVmr8GWSh7uDrEzX4rtwup4ps8JlCnGGtBg==\n";
    assert_eq!(
        signed_version(&encode(real.as_bytes())).as_deref(),
        Ok("0.2.0")
    );
}
