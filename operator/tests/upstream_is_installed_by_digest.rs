// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The pinned upstream agent-sandbox is installed with its controller named by the digest of the
//! pinned release. The vendored release manifest stays as upstream published it; beside it, a
//! kustomization records the controller image's tag and digest and is what the install applies
//! (`kubectl apply --server-side -k operator/vendor/agent-sandbox/<version>`). These tests fail
//! when that record and the pin disagree; `operator/e2e/kind.sh` checks, before it installs,
//! that the registry holds exactly the recorded digest under the pinned release's tag.

mod common;

use purlis_cluster_operator::upstream::{CONTROLLER_DIGEST, CONTROLLER_IMAGE, VERSION};
use serde_json::Value;
use std::path::PathBuf;

fn pinned() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("vendor/agent-sandbox")
        .join(VERSION)
}

fn record() -> Value {
    let documents = common::yaml_documents(&pinned().join("kustomization.yaml"));
    assert_eq!(documents.len(), 1, "{documents:?}");
    documents[0].clone()
}

#[test]
fn the_record_beside_the_pinned_release_names_its_controller_by_the_pinned_tag_and_a_digest() {
    let record = record();
    assert_eq!(record["kind"], "Kustomization", "{record}");
    // Only the vendored manifest, and nothing changed in it but the controller's image.
    assert_eq!(
        record["resources"],
        serde_json::json!(["sandbox-with-extensions.yaml"]),
        "{record}"
    );
    for key in record.as_object().expect("a mapping").keys() {
        assert!(
            matches!(key.as_str(), "apiVersion" | "kind" | "resources" | "images"),
            "the record changes {key}: {record}"
        );
    }
    let images = record["images"].as_array().expect("images");
    assert_eq!(images.len(), 1, "{record}");
    let image = &images[0];
    assert_eq!(image["name"], CONTROLLER_IMAGE, "{record}");
    assert_eq!(image["newTag"], VERSION, "{record}");
    assert_eq!(image["digest"], CONTROLLER_DIGEST, "{record}");
    let digest = CONTROLLER_DIGEST
        .strip_prefix("sha256:")
        .expect("a sha256 digest");
    assert!(
        digest.len() == 64
            && digest
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "{CONTROLLER_DIGEST}"
    );
}

#[test]
fn the_vendored_manifest_runs_the_controller_the_record_names_at_the_pinned_tag() {
    let manifest = common::yaml_documents(&pinned().join("sandbox-with-extensions.yaml"));
    let images: Vec<String> = manifest
        .iter()
        .filter(|document| document["kind"] == "Deployment")
        .flat_map(|deployment| {
            deployment["spec"]["template"]["spec"]["containers"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|container| container["image"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(images, [format!("{CONTROLLER_IMAGE}:{VERSION}")]);
}
