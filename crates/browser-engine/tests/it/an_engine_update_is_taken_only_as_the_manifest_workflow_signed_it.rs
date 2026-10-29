// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! An engine manifest that did not ship inside this build is trusted only through a Sigstore
//! bundle (#2073, ADR 0042 as amended): verified offline, against the trusted root this build
//! ships, from a certificate GitHub's token issuer gave to exactly one workflow of one repository
//! on one ref, with the signature's entry in the transparency log proven by the bundle.
//!
//! **The fixture is a real bundle**, not one made for this test: goreleaser's `checksums.txt` of
//! its v2.18.3 release and the bundle its release workflow made for it with keyless cosign on
//! GitHub Actions, published beside it (`tests/fixtures/sigstore/`). It was signed by
//! Sigstore's public-good instance, so it verifies against the same trusted root purlis ships.
//! purlis's own workflow makes bundles of the same kind, so what holds for this one holds for
//! them; only the identity differs, and that is what most of these tests turn.
//!
//! The staging root (`staging-trusted-root.json`) is Sigstore's staging instance's, copied from
//! the `sigstore-trust-root` crate: a real root of another instance.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use purlis_browser_engine::install::ManifestSignature;
use purlis_browser_engine::sigstore::{MANIFEST_WORKFLOW, Sigstore, TRUSTED_ROOT, Workflow};
use serde_json::Value;
use sha2::{Digest, Sha256};

const ARTIFACT: &[u8] = include_bytes!("../fixtures/sigstore/goreleaser-v2.18.3-checksums.txt");
const BUNDLE: &str =
    include_str!("../fixtures/sigstore/goreleaser-v2.18.3-checksums.txt.sigstore.json");
const STAGING_ROOT: &str = include_str!("../fixtures/sigstore/staging-trusted-root.json");

/// The workflow that signed the fixture.
const GORELEASER: Workflow<'static> = Workflow {
    repository: "goreleaser/goreleaser",
    file: "release.yml",
    git_ref: "refs/tags/v2.18.3",
};

fn trusting(workflow: &Workflow<'_>) -> Sigstore {
    Sigstore::trusting(TRUSTED_ROOT, workflow).expect("the shipped root reads")
}

/// The first log entry's inclusion proof in `bundle`.
fn first_proof(bundle: &mut Value) -> &mut Value {
    &mut bundle["verificationMaterial"]["tlogEntries"][0]["inclusionProof"]
}

fn bundle() -> Value {
    serde_json::from_str(BUNDLE).unwrap()
}

fn refused(signer: &Sigstore, artifact: &[u8], bundle: &str) -> String {
    signer
        .verify(artifact, bundle)
        .expect_err("the bundle must be refused")
}

#[test]
fn a_bundle_from_the_trusted_workflow_verifies_offline_against_the_shipped_root() {
    trusting(&GORELEASER).verify(ARTIFACT, BUNDLE).unwrap();
}

#[test]
fn the_identity_accepted_for_engine_manifests_is_the_manifest_workflow_on_main_of_purlis() {
    assert_eq!(
        Sigstore::engine_manifests().identity(),
        "https://github.com/purlis/purlis/.github/workflows/engine-manifest.yml@refs/heads/main"
    );
    assert_eq!(
        MANIFEST_WORKFLOW.identity(),
        Sigstore::engine_manifests().identity()
    );
}

#[test]
fn a_bundle_signed_by_any_other_identity_is_refused() {
    refused(&Sigstore::engine_manifests(), ARTIFACT, BUNDLE);
}

#[test]
fn a_bundle_of_another_repository_another_ref_or_another_workflow_is_refused() {
    for other in [
        Workflow {
            repository: "purlis/purlis",
            ..GORELEASER
        },
        Workflow {
            repository: "goreleaser/goreleaser-pro",
            ..GORELEASER
        },
        Workflow {
            git_ref: "refs/heads/main",
            ..GORELEASER
        },
        Workflow {
            git_ref: "refs/tags/v2.18.4",
            ..GORELEASER
        },
        Workflow {
            file: "engine-manifest.yml",
            ..GORELEASER
        },
    ] {
        refused(&trusting(&other), ARTIFACT, BUNDLE);
    }
}

#[test]
fn a_manifest_altered_after_signing_is_refused() {
    let mut altered = ARTIFACT.to_vec();
    altered[0] ^= 1;
    refused(&trusting(&GORELEASER), &altered, BUNDLE);
    // The bundle's own digest made to match the altered manifest: the signature still refuses.
    let mut rebound = bundle();
    rebound["messageSignature"]["messageDigest"]["digest"] =
        Value::String(BASE64.encode(Sha256::digest(&altered)));
    refused(&trusting(&GORELEASER), &altered, &rebound.to_string());
    let mut longer = ARTIFACT.to_vec();
    longer.extend_from_slice(b"\n");
    refused(&trusting(&GORELEASER), &longer, BUNDLE);
}

#[test]
fn a_bundle_with_no_log_entry_or_no_inclusion_proof_is_refused() {
    let mut none = bundle();
    none["verificationMaterial"]["tlogEntries"] = Value::Array(Vec::new());
    refused(&trusting(&GORELEASER), ARTIFACT, &none.to_string());

    let mut unproven = bundle();
    unproven["verificationMaterial"]["tlogEntries"][0]
        .as_object_mut()
        .unwrap()
        .remove("inclusionProof");
    refused(&trusting(&GORELEASER), ARTIFACT, &unproven.to_string());
}

/// Whatever format a bundle names, it is taken only as `cosign sign-blob` makes it (v0.3) and
/// only when every log entry in it carries its inclusion proof and a checkpoint.
#[test]
fn a_bundle_is_taken_only_in_the_v0_3_format_with_every_log_entry_proven() {
    for format in [
        "application/vnd.dev.sigstore.bundle+json;version=0.1",
        "application/vnd.dev.sigstore.bundle+json;version=0.2",
    ] {
        let mut older = bundle();
        older["mediaType"] = Value::String(format.to_owned());
        refused(&trusting(&GORELEASER), ARTIFACT, &older.to_string());
        older["verificationMaterial"]["tlogEntries"][0]
            .as_object_mut()
            .unwrap()
            .remove("inclusionProof");
        refused(&trusting(&GORELEASER), ARTIFACT, &older.to_string());
    }

    let mut second = bundle();
    let entries = second["verificationMaterial"]["tlogEntries"]
        .as_array_mut()
        .unwrap();
    let mut unproven = entries[0].clone();
    unproven.as_object_mut().unwrap().remove("inclusionProof");
    entries.push(unproven);
    refused(&trusting(&GORELEASER), ARTIFACT, &second.to_string());

    let mut no_checkpoint = bundle();
    first_proof(&mut no_checkpoint)["checkpoint"]["envelope"] = Value::String(String::new());
    refused(&trusting(&GORELEASER), ARTIFACT, &no_checkpoint.to_string());
}

#[test]
fn a_bundle_whose_inclusion_proof_or_checkpoint_was_altered_is_refused() {
    let mut hashes = bundle();
    let first = &mut first_proof(&mut hashes)["hashes"][0];
    let mut bytes = first.as_str().unwrap().to_owned().into_bytes();
    bytes[0] = if bytes[0] == b'A' { b'B' } else { b'A' };
    *first = Value::String(String::from_utf8(bytes).unwrap());
    refused(&trusting(&GORELEASER), ARTIFACT, &hashes.to_string());

    // The log's signature on the checkpoint, its last line, with one character changed.
    let mut checkpoint = bundle();
    let envelope = &mut first_proof(&mut checkpoint)["checkpoint"]["envelope"];
    let mut text = envelope.as_str().unwrap().as_bytes().to_vec();
    let at = text.iter().rposition(|&b| b == b' ').unwrap() + 12;
    text[at] = if text[at] == b'A' { b'B' } else { b'A' };
    *envelope = Value::String(String::from_utf8(text).unwrap());
    refused(&trusting(&GORELEASER), ARTIFACT, &checkpoint.to_string());
}

/// The shipped root with every transparency log's key ended before the fixture was logged
/// (2026-10-09): the proof is one the root no longer vouches for.
#[test]
fn a_proof_from_a_log_key_that_had_expired_when_it_was_logged_is_refused() {
    let mut root: Value = serde_json::from_str(TRUSTED_ROOT).unwrap();
    for log in root["tlogs"].as_array_mut().unwrap() {
        log["publicKey"]["validFor"]["end"] = Value::String("2026-01-01T00:00:00Z".to_owned());
    }
    let signer = Sigstore::trusting(&root.to_string(), &GORELEASER).unwrap();
    refused(&signer, ARTIFACT, BUNDLE);
}

/// The shipped root with its certificate authorities ended before the fixture was signed.
#[test]
fn a_certificate_from_an_authority_that_had_expired_when_it_signed_is_refused() {
    let mut root: Value = serde_json::from_str(TRUSTED_ROOT).unwrap();
    for authority in root["certificateAuthorities"].as_array_mut().unwrap() {
        authority["validFor"]["end"] = Value::String("2026-01-01T00:00:00Z".to_owned());
    }
    let signer = Sigstore::trusting(&root.to_string(), &GORELEASER).unwrap();
    refused(&signer, ARTIFACT, BUNDLE);
}

#[test]
fn a_bundle_checked_against_another_instances_root_is_refused() {
    let signer = Sigstore::trusting(STAGING_ROOT, &GORELEASER).unwrap();
    refused(&signer, ARTIFACT, BUNDLE);
}

#[test]
fn what_is_not_a_bundle_is_refused() {
    for text in [
        "",
        "{}",
        "not json",
        "untrusted comment: a minisign signature\nRWQ\n",
    ] {
        refused(&trusting(&GORELEASER), ARTIFACT, text);
    }
}

#[test]
fn a_root_that_does_not_read_is_refused_when_the_verifier_is_made() {
    assert!(Sigstore::trusting("{}", &GORELEASER).is_err());
    assert!(Sigstore::trusting("not json", &GORELEASER).is_err());
}
