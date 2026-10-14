// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's own kubeconfig for a cluster on this Mac (qw384b): one context copied, its server
//! named as the proxy carries it, the certificate still checked against the name it was made for.

use serde_json::{Value, json};

use super::{ON_THIS_MAC, contexts, for_this_chat, on_this_mac};

/// What `kubectl config view --raw --minify --context <name> -o json` prints for one context.
fn minified(name: &str, server: &str) -> Value {
    json!({
        "kind": "Config",
        "apiVersion": "v1",
        "preferences": {},
        "clusters": [{"name": name, "cluster": {
            "server": server,
            "certificate-authority-data": "Q0EtREFUQQ==",
        }}],
        "users": [{"name": name, "user": {"client-certificate": "/Users/someone/.kube/kind.crt"}}],
        "contexts": [{"name": name, "context": {"cluster": name, "user": name, "namespace": "dev"}}],
        "current-context": name,
    })
}

#[test]
fn the_servers_purlis_points_at_are_this_macs_by_the_three_names_alone() {
    assert_eq!(
        ON_THIS_MAC,
        ["127.0.0.1", "localhost", "kubernetes.docker.internal"]
    );
    assert_eq!(
        on_this_mac("https://127.0.0.1:50795"),
        Ok(("127.0.0.1".to_owned(), 50795))
    );
    assert_eq!(
        on_this_mac("https://LOCALHOST:6443/"),
        Ok(("localhost".to_owned(), 6443))
    );
    assert_eq!(
        on_this_mac("https://kubernetes.docker.internal:6443"),
        Ok(("kubernetes.docker.internal".to_owned(), 6443))
    );
    // No port: HTTPS's own.
    assert_eq!(
        on_this_mac("https://localhost"),
        Ok(("localhost".to_owned(), 443))
    );
    for elsewhere in [
        "https://10.0.0.5:6443",
        "https://api.prod.example.com",
        "https://127.0.0.2:6443",
        "https://192.168.65.3:6443",
        "https://localhost.example.com:6443",
        "http://127.0.0.1:8080",
        "not a url",
    ] {
        let why = on_this_mac(elsewhere).expect_err(elsewhere);
        assert!(why.contains("on this Mac"), "{elsewhere}: {why}");
    }
}

#[test]
fn the_copy_names_this_mac_through_the_proxy_and_checks_the_certificate_against_the_old_name() {
    let pointed = for_this_chat(
        &minified("kind-kind", "https://127.0.0.1:50795"),
        "kind-kind",
    )
    .expect("pointed");
    assert_eq!(pointed.port, 50795);
    let config: Value = serde_json::from_str(&pointed.config).expect("JSON, which kubectl reads");
    let cluster = &config["clusters"][0]["cluster"];
    assert_eq!(cluster["server"], "https://this-mac.internal:50795");
    assert_eq!(cluster["tls-server-name"], "127.0.0.1");
    // Everything else as kubectl printed it: the authority, the user, the namespace.
    assert_eq!(cluster["certificate-authority-data"], "Q0EtREFUQQ==");
    assert_eq!(
        config["users"][0]["user"]["client-certificate"],
        "/Users/someone/.kube/kind.crt"
    );
    assert_eq!(config["contexts"][0]["context"]["namespace"], "dev");
    assert_eq!(config["current-context"], "kind-kind");
}

#[test]
fn a_tls_server_name_the_context_already_names_is_kept() {
    let mut view = minified("dd", "https://kubernetes.docker.internal:6443");
    view["clusters"][0]["cluster"]["tls-server-name"] = json!("kubernetes");
    let pointed = for_this_chat(&view, "dd").expect("pointed");
    let config: Value = serde_json::from_str(&pointed.config).expect("JSON");
    assert_eq!(
        config["clusters"][0]["cluster"]["tls-server-name"],
        "kubernetes"
    );
    assert_eq!(
        config["clusters"][0]["cluster"]["server"],
        "https://this-mac.internal:6443"
    );
}

#[test]
fn a_context_whose_server_is_not_on_this_mac_is_refused_with_why() {
    let why = for_this_chat(&minified("prod", "https://api.prod.example.com"), "prod")
        .expect_err("refused");
    assert!(why.contains("prod"), "{why}");
    assert!(why.contains("api.prod.example.com"), "{why}");
    assert!(why.contains("on this Mac"), "{why}");
}

#[test]
fn a_context_that_names_a_proxy_of_its_own_or_skips_the_certificate_check_is_refused() {
    let mut proxied = minified("kind", "https://127.0.0.1:50795");
    proxied["clusters"][0]["cluster"]["proxy-url"] = json!("http://proxy.example:3128");
    let why = for_this_chat(&proxied, "kind").expect_err("refused");
    assert!(why.contains("proxy"), "{why}");

    let mut unchecked = minified("kind", "https://127.0.0.1:50795");
    unchecked["clusters"][0]["cluster"]["insecure-skip-tls-verify"] = json!(true);
    let why = for_this_chat(&unchecked, "kind").expect_err("refused");
    assert!(why.contains("certificate"), "{why}");
}

#[test]
fn a_view_that_is_not_the_one_context_asked_for_is_refused() {
    let why = for_this_chat(&minified("other", "https://127.0.0.1:1"), "kind").expect_err("no");
    assert!(why.contains("kind"), "{why}");
    let why = for_this_chat(&json!({"clusters": []}), "kind").expect_err("no");
    assert!(why.contains("kind"), "{why}");
}

#[test]
fn every_context_is_listed_with_its_server_and_whether_purlis_can_point_at_it() {
    let view = json!({
        "clusters": [
            {"name": "kind-kind", "cluster": {"server": "https://127.0.0.1:50795"}},
            {"name": "prod", "cluster": {"server": "https://api.prod.example.com"}},
        ],
        "contexts": [
            {"name": "kind-kind", "context": {"cluster": "kind-kind", "user": "u"}},
            {"name": "prod", "context": {"cluster": "prod", "user": "u"}},
            {"name": "orphan", "context": {"cluster": "gone", "user": "u"}},
        ],
    });
    let listed = contexts(&view);
    assert_eq!(listed.len(), 3);
    assert_eq!(listed[0].name, "kind-kind");
    assert_eq!(listed[0].server, "https://127.0.0.1:50795");
    assert_eq!(listed[0].port, Some(50795));
    assert_eq!(listed[0].refused, None);
    assert_eq!(listed[1].name, "prod");
    assert_eq!(listed[1].port, None);
    assert!(
        listed[1]
            .refused
            .as_deref()
            .is_some_and(|why| why.contains("on this Mac")),
        "{:?}",
        listed[1].refused
    );
    assert_eq!(listed[2].port, None);
    assert!(listed[2].refused.is_some());
}
