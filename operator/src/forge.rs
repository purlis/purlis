// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Pool opted in to minting GitHub App tokens in the cluster** (#4204, the rulings on #3807).
//!
//! A cluster admin keeps an organisation's App key as a Secret in the operator's own namespace,
//! names it in the chart's `forge.keySecretNames`, which is all the Role
//! `purlis-forge-key-reader` lets the operator `get`, and names it again on a Pool with
//! `spec.forge.mintInCluster: true`. As it looks at such a Pool the operator reads that Secret
//! and says in its log whether the Pool can mint: a Pool that is not opted in has none of its
//! Secrets read. Nothing is minted yet, and the key is not kept: #4206 mints.

use crate::resources::PoolSpec;
use k8s_openapi::api::core::v1::Secret;
use kube::Api;
use std::collections::HashMap;

/// What `Pool.spec.forge.repos` holds: `owner/name` or `owner/*`, never a bare `*` or `*/*`, a
/// partial wildcard, a path or a name of dots alone.
pub const POOL_REPO: &str =
    r"^[A-Za-z0-9][A-Za-z0-9_-]{0,38}/(\*|[A-Za-z0-9._-]*[A-Za-z0-9_-][A-Za-z0-9._-]*)$";

/// What `Claim.spec.forge.repos` holds: `owner/name` alone. A wildcard here would make the
/// intersection with a Pool's `owner/*` every repo of the owner, whatever the workspace holds.
pub const CLAIM_REPO: &str =
    r"^[A-Za-z0-9][A-Za-z0-9_-]{0,38}/[A-Za-z0-9._-]*[A-Za-z0-9_-][A-Za-z0-9._-]*$";

/// A Secret's name as Kubernetes spells one.
pub const SECRET_NAME: &str = r"^[a-z0-9]([-a-z0-9.]*[a-z0-9])?$";

/// The operator's own signing key, which no Pool may name and no value may let it read: its pod
/// has it from the kubelet, and whoever reads it vouches for any runner.
pub const SIGNING_KEY_SECRET: &str = "purlis-cluster-operator-signing-key";

/// The Secret's data key holding the App's client id.
pub const APP: &str = "app";
/// The Secret's data key holding the App's private key, as a PEM.
pub const KEY: &str = "key";

/// The most a client id may be, as the desktop's tool lines hold it (`mint::of`).
const MOST_APP: usize = 128;

/// The Secret to read for this Pool: only one that says `mintInCluster`.
pub fn key_to_read(pool: &PoolSpec) -> Option<&str> {
    pool.forge
        .as_ref()
        .filter(|forge| forge.mint_in_cluster)
        .map(|forge| forge.app_key_secret_ref.name.as_str())
}

/// An App key's Secret, read and found whole. It keeps the App's client id alone. Neither a
/// refusal nor its `Debug` holds anything of the Secret but that client id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppKey {
    app: String,
}

impl AppKey {
    /// `secret`'s client id, once it holds a client id under [`APP`] and a PEM under [`KEY`].
    pub fn of(secret: &Secret) -> Result<AppKey, String> {
        let held = |key: &str| secret.data.as_ref().and_then(|data| data.get(key));
        let app = held(APP)
            .and_then(|app| std::str::from_utf8(&app.0).ok())
            .filter(|app| {
                !app.is_empty()
                    && app.len() <= MOST_APP
                    && app
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            })
            .ok_or_else(|| format!("its data key {APP} holds no GitHub App's client id"))?;
        if !held(KEY).is_some_and(|key| key.0.starts_with(b"-----BEGIN ")) {
            return Err(format!("its data key {KEY} holds no PEM private key"));
        }
        Ok(AppKey {
            app: app.to_owned(),
        })
    }

    /// The App's client id.
    pub fn app(&self) -> &str {
        &self.app
    }

    /// Ruling Q6 on #3807: a persona tool line naming another App than the Secret's is refused.
    /// The operator holds the Secret, so it is the one to say: #4206's endpoint asks this of the
    /// `app` a runner's tool line names.
    pub fn serves(&self, app: &str) -> Result<(), String> {
        if app == self.app {
            return Ok(());
        }
        Err(format!(
            "the tool names the GitHub App {app}, and this pool's key is the App {}",
            self.app
        ))
    }
}

/// **Says in the log whether each opted-in Pool can mint**, once per thing it has to say, so a
/// Pool looked at again says nothing new. Read by name, with the operator's own right to `get`
/// what the chart names, in its own namespace.
#[derive(Default)]
pub struct KeyChecks {
    said: HashMap<String, Result<String, String>>,
}

impl KeyChecks {
    /// Looks at the Pool `name`, whose `spec` the cluster holds.
    pub async fn look_at(&mut self, secrets: &Api<Secret>, name: &str, spec: &PoolSpec) {
        let Some(secret) = key_to_read(spec) else {
            self.said.remove(name);
            return;
        };
        let read = match secrets.get(secret).await {
            Ok(found) => AppKey::of(&found).map(|key| key.app),
            Err(kube::Error::Api(status)) if status.is_forbidden() => Err(
                "the chart's forge.keySecretNames does not name it, so the operator may not read it"
                    .to_owned(),
            ),
            Err(kube::Error::Api(status)) if status.is_not_found() => {
                Err("it is not in the operator's namespace".to_owned())
            }
            Err(error) => Err(error.to_string()),
        };
        if self.said.get(name) == Some(&read) {
            return;
        }
        match &read {
            Ok(app) => {
                tracing::info!(pool = %name, %secret, %app, "the pool's App key is readable")
            }
            Err(why) => {
                tracing::warn!(pool = %name, %secret, %why, "the pool's App key cannot be read")
            }
        }
        self.said.insert(name.to_owned(), read);
    }
}
