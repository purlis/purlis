// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A cluster admin opts a Pool in to minting GitHub App tokens in the cluster (#4204, rulings on
//! #3807): what the definitions hold of it, and how the operator reads the admin's Secret. The
//! API server's own refusals are on the kind job (`on_a_kind_cluster.rs`).

use k8s_openapi::ByteString;
use k8s_openapi::api::core::v1::Secret;
use kube::CustomResourceExt;
use purlis_cluster_operator::forge::{self, AppKey};
use purlis_cluster_operator::{Claim, ClaimForge, ClaimSpec, Pool, PoolForge, PoolSpec, SecretRef};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const IMAGE: &str =
    "example.test/runner@sha256:0000000000000000000000000000000000000000000000000000000000000000";
mod app_key;

fn forge_schema(
    crd: &k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
) -> Value {
    let schema = serde_json::to_value(crd).expect("JSON");
    schema["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]["properties"]
        ["forge"]
        .clone()
}

#[test]
fn a_pool_says_nothing_of_the_forge_unless_it_is_told_to() {
    let spec = serde_json::to_value(PoolSpec::new(IMAGE)).expect("JSON");
    assert!(spec.get("forge").is_none(), "{spec}");
    let read: PoolSpec = serde_json::from_value(json!({
        "image": IMAGE,
        "forge": { "appKeySecretRef": { "name": "org-app" }, "repos": ["acme/*"] },
    }))
    .expect("a pool");
    let opted = read.forge.expect("its forge");
    assert_eq!(
        opted.app_key_secret_ref,
        SecretRef {
            name: "org-app".into()
        }
    );
    // Naming a key is not yet minting: only `mintInCluster` says so.
    assert!(!opted.mint_in_cluster);
}

#[test]
fn a_pools_allow_list_and_its_key_are_required_and_spelled_as_ruled() {
    let crd = Pool::crd();
    let forge = forge_schema(&crd);
    let required = forge["required"].as_array().expect("required keys");
    assert!(required.contains(&json!("appKeySecretRef")), "{forge}");
    assert!(required.contains(&json!("repos")), "{forge}");
    let props = &forge["properties"];
    assert_eq!(props["mintInCluster"]["type"], "boolean", "{forge}");
    assert_eq!(props["repos"]["minItems"], 1, "{forge}");
    assert_eq!(
        props["repos"]["items"]["pattern"],
        forge::POOL_REPO,
        "{forge}"
    );
    assert!(
        props["appKeySecretRef"]["properties"]
            .get("namespace")
            .is_none()
    );
    let name = &props["appKeySecretRef"]["properties"]["name"];
    assert_eq!(name["pattern"], forge::SECRET_NAME, "{forge}");
    assert!(
        name["x-kubernetes-validations"]
            .to_string()
            .contains(forge::SIGNING_KEY_SECRET),
        "a Pool may name the operator's own signing key: {forge}"
    );
}

#[test]
fn a_claim_names_exact_repos_and_never_a_wildcard() {
    let crd = Claim::crd();
    let forge = forge_schema(&crd);
    let pattern = forge["properties"]["repos"]["items"]["pattern"]
        .as_str()
        .expect("a pattern");
    assert_eq!(pattern, forge::CLAIM_REPO);
    assert!(!pattern.contains("\\*"), "{pattern}");
    // Left out, a Claim says nothing, and the spec reads as it did before.
    let spec: ClaimSpec =
        serde_json::from_value(json!({ "pool": "team", "requester": "ada" })).expect("a claim");
    assert_eq!(spec.forge, None);
    let said = ClaimForge {
        repos: vec!["acme/app".into()],
    };
    assert_eq!(
        serde_json::to_value(said).unwrap(),
        json!({ "repos": ["acme/app"] })
    );
}

#[test]
fn only_a_pool_that_says_mint_in_cluster_is_opted_in() {
    let forge_of = |mint| PoolForge {
        app_key_secret_ref: SecretRef {
            name: "org-app".into(),
        },
        repos: vec!["acme/*".into()],
        mint_in_cluster: mint,
    };
    let mut pool = PoolSpec::new(IMAGE);
    assert_eq!(forge::key_to_read(&pool), None);
    pool.forge = Some(forge_of(false));
    assert_eq!(forge::key_to_read(&pool), None);
    pool.forge = Some(forge_of(true));
    assert_eq!(forge::key_to_read(&pool), Some("org-app"));
}

fn secret(data: &[(&str, &str)]) -> Secret {
    Secret {
        data: Some(
            data.iter()
                .map(|(k, v)| ((*k).to_owned(), ByteString(v.as_bytes().to_vec())))
                .collect::<BTreeMap<_, _>>(),
        ),
        ..Secret::default()
    }
}

#[test]
fn the_secret_holds_the_apps_client_id_and_its_pem() {
    let pem = app_key::pem();
    let key = AppKey::of(&secret(&[("app", "Iv23liExample"), ("key", &pem)])).expect("a key");
    assert_eq!(key.app(), "Iv23liExample");
    // A PEM of something else is no key: it is parsed before it is called usable (#4206).
    let label = ["PRIVATE ", "KEY"].concat();
    let not_rsa = format!("-----BEGIN {label}-----\naHVudGVyMg==\n-----END {label}-----\n");
    for (data, says) in [
        (vec![("key", pem.as_str())], "app"),
        (vec![("app", "Iv23liExample")], "key"),
        (vec![("app", "not a client id"), ("key", &pem)], "app"),
        (vec![("app", "Iv23liExample"), ("key", "hunter2")], "key"),
        (
            vec![("app", "Iv23liExample"), ("key", not_rsa.as_str())],
            "key",
        ),
    ] {
        let why = AppKey::of(&secret(&data)).expect_err("refused");
        assert!(why.contains(says), "{why}");
        assert!(
            !why.contains("hunter2")
                && !why.contains("aHVudGVyMg")
                && !why.contains("not a client id"),
            "{why}"
        );
    }
    assert!(AppKey::of(&Secret::default()).is_err());
}

/// Ruling Q6: a persona tool line whose `app` is not the Secret's is refused.
#[test]
fn a_tool_line_naming_another_app_is_refused() {
    let pem = app_key::pem();
    let key = AppKey::of(&secret(&[("app", "Iv23liExample"), ("key", &pem)])).expect("a key");
    assert_eq!(key.serves("Iv23liExample"), Ok(()));
    let why = key.serves("Iv23liOther").expect_err("refused");
    assert!(
        why.contains("Iv23liOther") && why.contains("Iv23liExample"),
        "{why}"
    );
    // A client id is compared exactly: GitHub's are case-sensitive.
    assert!(key.serves("iv23liexample").is_err());
}

#[test]
fn what_the_operator_logs_of_a_key_never_holds_the_key() {
    let pem = app_key::pem();
    let key = AppKey::of(&secret(&[("app", "Iv23liExample"), ("key", &pem)])).expect("a key");
    let printed = format!("{key:?}");
    assert!(printed.contains("Iv23liExample"), "{printed}");
    assert!(
        !printed.contains("MII") && !printed.contains("KEY"),
        "{printed}"
    );
}
