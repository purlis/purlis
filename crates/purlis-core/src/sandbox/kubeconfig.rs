// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's own kubeconfig for a cluster on this Mac** (qw384b).
//!
//! kubectl, like every Go client, never sends `127.0.0.1` or `localhost` to a proxy, and a
//! Claude Code chat's commands send no address of this machine to one either (its `NO_PROXY`),
//! so a cluster at `https://127.0.0.1:50795` is refused by the chat's sandbox however its port
//! is allowed. Named [`super::localport::THIS_MAC`] instead, the connection goes to the chat's
//! proxy, which carries it to that port while the chat holds it
//! ([`super::reach::Reach::decide_resolving`]).
//!
//! On the person's press, the window copies one context for one chat ([`for_this_chat`]):
//!
//! - **Only a cluster on this Mac** ([`ON_THIS_MAC`]): `127.0.0.1`, `localhost` or
//!   `kubernetes.docker.internal`, on HTTPS. Any other is refused with why: a remote cluster is
//!   a host, allowed as one.
//! - **Its server named as the proxy carries it**: `https://this-mac.internal:<port>`, and
//!   `tls-server-name` the name it had, so kubectl still checks the cluster's certificate
//!   against the name it was made for. A context that already names one keeps it.
//! - **Nothing else changes**: the authority, the user and the namespace, as kubectl printed
//!   them. A context that names a proxy of its own, or skips the certificate check, is refused:
//!   the first would not reach the chat's proxy, the second would carry anything at that port.
//! - **The person's own kubeconfig is never written**: the copy is the chat's, in a folder of
//!   its own, named by `KUBECONFIG` for that chat alone, and gone when the chat ends.
//!
//! What it copies from is `kubectl config view --raw --minify --context <name> -o json`, which
//! reads every file `KUBECONFIG` names as kubectl itself does and makes each path it names
//! absolute. JSON is a kubeconfig kubectl reads.

use serde_json::Value;

/// The names of this machine a cluster's server may have for purlis to point a chat at it.
pub const ON_THIS_MAC: [&str; 3] = ["127.0.0.1", "localhost", "kubernetes.docker.internal"];

/// The file's name in the chat's own folder.
pub const FILE: &str = "kubeconfig.json";

/// A context copied for one chat: the kubeconfig to write, and the port of this Mac it reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pointed {
    pub config: String,
    pub port: u16,
}

/// One context of a kubeconfig, as the window lists it: by name, with its cluster's server, and
/// the port of this Mac it reaches or why purlis would not point a chat at it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    pub name: String,
    pub server: String,
    pub port: Option<u16>,
    pub refused: Option<String>,
}

/// **`server` as a cluster on this Mac**: its host, as one of [`ON_THIS_MAC`], and its port; or
/// why not.
pub fn on_this_mac(server: &str) -> Result<(String, u16), String> {
    let not = || {
        format!(
            "{server} is not a cluster on this Mac: purlis points a chat's kubectl only at \
             https://127.0.0.1, https://localhost or https://kubernetes.docker.internal. A \
             cluster elsewhere is a host, which the chat's Notice can allow."
        )
    };
    let rest = server.trim().strip_prefix("https://").ok_or_else(not)?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
        return Err(not());
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (
            host,
            port.parse::<u16>()
                .ok()
                .filter(|port| *port != 0)
                .ok_or_else(not)?,
        ),
        None => (authority, 443),
    };
    let host = host.to_ascii_lowercase();
    if ON_THIS_MAC.contains(&host.as_str()) {
        Ok((host, port))
    } else {
        Err(not())
    }
}

/// **`context` of `view`, copied for one chat** ([`self`]), or why not. `view` is what
/// `kubectl config view --raw --minify --context <context> -o json` printed.
pub fn for_this_chat(view: &Value, context: &str) -> Result<Pointed, String> {
    let missing = || format!("purlis found no context named {context} in your kubeconfig.");
    let named = |list: &str, name: &str| -> Option<Value> {
        view.get(list)?
            .as_array()?
            .iter()
            .find(|one| one.get("name").and_then(Value::as_str) == Some(name))
            .cloned()
    };
    let found = named("contexts", context).ok_or_else(missing)?;
    let cluster_name = found
        .pointer("/context/cluster")
        .and_then(Value::as_str)
        .ok_or_else(missing)?;
    let mut cluster = named("clusters", cluster_name).ok_or_else(missing)?;
    let server = cluster
        .pointer("/cluster/server")
        .and_then(Value::as_str)
        .ok_or_else(missing)?
        .to_owned();
    let (host, port) = on_this_mac(&server).map_err(|why| format!("{context}: {why}"))?;
    let inner = cluster
        .get_mut("cluster")
        .and_then(Value::as_object_mut)
        .ok_or_else(missing)?;
    if inner.contains_key("proxy-url") {
        return Err(format!(
            "{context} names a proxy of its own, so its connections would not go through this \
             chat's proxy: purlis does not point a chat at it."
        ));
    }
    if inner
        .get("insecure-skip-tls-verify")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(format!(
            "{context} skips the cluster's certificate check, so it would trust whatever \
             listens at that port: purlis does not point a chat at it."
        ));
    }
    inner.insert(
        "server".to_owned(),
        Value::String(format!("https://{}:{port}", super::localport::THIS_MAC)),
    );
    inner
        .entry("tls-server-name")
        .or_insert_with(|| Value::String(host));
    let user = found
        .pointer("/context/user")
        .and_then(Value::as_str)
        .and_then(|name| named("users", name));
    let config = serde_json::json!({
        "kind": "Config",
        "apiVersion": "v1",
        "preferences": {},
        "clusters": [cluster],
        "users": user.into_iter().collect::<Vec<_>>(),
        "contexts": [found],
        "current-context": context,
    });
    Ok(Pointed {
        config: serde_json::to_string_pretty(&config).map_err(|why| why.to_string())?,
        port,
    })
}

/// **Every context `view` holds** (`kubectl config view -o json`), in its order, each with its
/// cluster's server and the port of this Mac it reaches, or why purlis would not point a chat
/// at it.
pub fn contexts(view: &Value) -> Vec<Context> {
    let list = |name: &str| {
        view.get(name)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let clusters = list("clusters");
    list("contexts")
        .iter()
        .filter_map(|one| {
            let name = one.get("name")?.as_str()?.to_owned();
            let server = one
                .pointer("/context/cluster")
                .and_then(Value::as_str)
                .and_then(|cluster| {
                    clusters
                        .iter()
                        .find(|c| c.get("name").and_then(Value::as_str) == Some(cluster))
                })
                .and_then(|cluster| cluster.pointer("/cluster/server"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            let Some(server) = server else {
                return Some(Context {
                    refused: Some(format!(
                        "{name} names no cluster your kubeconfig holds, so purlis cannot point a \
                         chat at it."
                    )),
                    name,
                    server: String::new(),
                    port: None,
                });
            };
            let (port, refused) = match on_this_mac(&server) {
                Ok((_, port)) => (Some(port), None),
                Err(why) => (None, Some(why)),
            };
            Some(Context {
                name,
                server,
                port,
                refused,
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "kubeconfig_tests.rs"]
mod tests;
