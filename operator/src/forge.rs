// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Pool opted in to minting GitHub App tokens in the cluster** (#4204, the rulings on #3807).
//!
//! A cluster admin keeps an organisation's App key as a Secret in the operator's own namespace,
//! names it in the chart's `forge.keySecretNames`, which is all the Role
//! `purlis-forge-key-reader` lets the operator `get`, and names it again on a Pool with
//! `spec.forge.mintInCluster: true`. As it looks at such a Pool the operator reads that Secret
//! and says in its log whether the Pool can mint: a Pool that is not opted in has none of its
//! Secrets read. The key is read again for each mint and never kept ([`crate::mint`], #4206).

use crate::resources::{Pool, PoolSpec};
use base64::Engine as _;
use k8s_openapi::api::core::v1::Secret;
use kube::core::Request;
use kube::{Client, ResourceExt};
use ring::signature::RsaKeyPair;
use std::collections::{BTreeSet, HashMap};

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

/// The RBAC resource a person is granted `use` on, by the name of the Secret, to point a Pool at
/// that key (ruled on #4206). The cluster holds no such object: it is a name for a right, asked of
/// the cluster's authorizer as `use` on a Pool is.
pub const KEY_RESOURCE: &str = "forgekeys";

/// The NetworkPolicy the operator makes in a runner namespace with a Claim on an opted-in Pool:
/// its runners may reach the operator's mint port, on top of what upstream's own policy allows.
pub const MINT_EGRESS_POLICY: &str = "purlis-forge-mint";

/// The most repos one token is limited to: GitHub refuses a longer `repositories` list.
pub const MOST_REPOS: usize = 500;

/// The most a client id may be, as the desktop's tool lines hold it (`mint::of`).
const MOST_APP: usize = 64;

/// How long one read of a Secret is waited for: a stalled read says so and is tried at the next
/// look, rather than hold up every other Pool's.
const READ_WITHIN: std::time::Duration = std::time::Duration::from_secs(10);

/// The Secret to read for this Pool: only one that says `mintInCluster`.
pub fn key_to_read(pool: &PoolSpec) -> Option<&str> {
    pool.forge
        .as_ref()
        .filter(|forge| forge.mint_in_cluster)
        .map(|forge| forge.app_key_secret_ref.name.as_str())
}

/// An App key's Secret, read and found whole: its client id, and its private key parsed. Neither a
/// refusal nor its `Debug` holds anything of the Secret but that client id.
#[derive(Clone)]
pub struct AppKey {
    app: String,
    pair: std::sync::Arc<RsaKeyPair>,
}

impl std::fmt::Debug for AppKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppKey").field("app", &self.app).finish()
    }
}

/// The PEM `held` holds as DER, and whether it is PKCS#8 rather than the PKCS#1 GitHub hands out.
fn der_of(held: &[u8]) -> Option<(Vec<u8>, bool)> {
    let text = std::str::from_utf8(held).ok()?.trim();
    let armour = |label: &str| {
        (
            format!("-----BEGIN {label}-----"),
            format!("-----END {label}-----"),
        )
    };
    let [pkcs1, pkcs8] = [
        ["RSA ", "PRIVATE KEY"].concat(),
        ["PRIVATE ", "KEY"].concat(),
    ];
    [(pkcs1, false), (pkcs8, true)]
        .into_iter()
        .find_map(|(label, is_pkcs8)| {
            let (begin, end) = armour(&label);
            let body = text.strip_prefix(&begin)?.strip_suffix(&end)?;
            let body: String = body.split_whitespace().collect();
            let der = base64::engine::general_purpose::STANDARD
                .decode(body)
                .ok()?;
            Some((der, is_pkcs8))
        })
}

impl AppKey {
    /// `secret`'s client id and key, once it holds a client id under [`APP`] and, under [`KEY`], a
    /// PEM that parses as an RSA private key.
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
        let (der, pkcs8) = held(KEY)
            .and_then(|key| der_of(&key.0))
            .ok_or_else(|| format!("its data key {KEY} holds no PEM private key"))?;
        let pair = if pkcs8 {
            RsaKeyPair::from_pkcs8(&der)
        } else {
            RsaKeyPair::from_der(&der)
        }
        .map_err(|_| format!("its data key {KEY} holds no RSA private key GitHub would take"))?;
        Ok(AppKey {
            app: app.to_owned(),
            pair: std::sync::Arc::new(pair),
        })
    }

    /// The App's client id.
    pub fn app(&self) -> &str {
        &self.app
    }

    /// Ruling Q6 on #3807: a persona tool line naming another App than the Secret's is refused.
    /// The operator holds the Secret, so it is the one to say: its endpoint asks this of the `app`
    /// a runner's tool line names ([`crate::mint`]).
    pub fn serves(&self, app: &str) -> Result<(), String> {
        if app == self.app {
            return Ok(());
        }
        Err(format!(
            "the tool names the GitHub App {app}, and this pool's key is the App {}",
            self.app
        ))
    }

    /// A JWT for the App, signed RS256, issued a minute before `now` (seconds since 1970) and good
    /// for nine minutes, as GitHub's app endpoints take one. It is the bearer of one mint.
    pub fn jwt(&self, now: u64) -> Result<String, String> {
        let url = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let header = url.encode(br#"{"alg":"RS256","typ":"JWT"}"#);
        let claims = serde_json::json!({
            "iat": now.saturating_sub(60),
            "exp": now + 9 * 60,
            "iss": self.app,
        });
        let signed = format!("{header}.{}", url.encode(claims.to_string().as_bytes()));
        let mut signature = vec![0u8; self.pair.public().modulus_len()];
        self.pair
            .sign(
                &ring::signature::RSA_PKCS1_SHA256,
                &ring::rand::SystemRandom::new(),
                signed.as_bytes(),
                &mut signature,
            )
            .map_err(|_| "the GitHub App's key could not sign a token request".to_owned())?;
        Ok(format!("{signed}.{}", url.encode(&signature)))
    }
}

/// What a mint covers: the repos of one owner, as the Claim spells them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Covered {
    pub owner: String,
    /// The repos' names, without their owner, each once.
    pub names: Vec<String>,
}

/// Whether `repo`, `owner/name`, is on the allow-list entry `entry`, `owner/name` or `owner/*`.
/// GitHub's names do not tell case apart, so neither does this.
fn allowed_by(entry: &str, repo: &str) -> bool {
    let (Some((allowed_owner, allowed_name)), Some((owner, _))) =
        (entry.split_once('/'), repo.split_once('/'))
    else {
        return false;
    };
    allowed_owner.eq_ignore_ascii_case(owner)
        && (allowed_name == "*" || entry.eq_ignore_ascii_case(repo))
}

/// Whether `word` could be a part of a repo's path: what is put in a URL is only ever this.
fn plain_path_part(word: &str) -> bool {
    !word.is_empty()
        && word.bytes().any(|b| b != b'.')
        && word
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// **What a token may cover**: the repos of the Claim that the Pool's allow-list holds. Refused,
/// so nothing is minted, where that is no repo at all (GitHub reads an omitted list as every repo
/// of the installation), where it is more than one owner (one token is of one installation), or
/// more than GitHub takes.
pub fn covered(allow_list: &[String], claim: &[String]) -> Result<Covered, String> {
    if claim.is_empty() {
        return Err("the claim names no repos".to_owned());
    }
    let mut seen = BTreeSet::new();
    let mut owners = BTreeSet::new();
    let mut covered: Option<Covered> = None;
    for repo in claim {
        let Some((owner, name)) = repo.split_once('/') else {
            continue;
        };
        if !plain_path_part(owner)
            || !plain_path_part(name)
            || !allow_list.iter().any(|entry| allowed_by(entry, repo))
            || !seen.insert(repo.to_ascii_lowercase())
        {
            continue;
        }
        owners.insert(owner.to_ascii_lowercase());
        let held = covered.get_or_insert_with(|| Covered {
            owner: owner.to_owned(),
            names: Vec::new(),
        });
        held.names.push(name.to_owned());
    }
    if owners.len() > 1 {
        return Err(format!(
            "the claim's repos on the pool's allow-list are of {} owners, and one token is of one installation",
            owners.len()
        ));
    }
    let covered = covered.ok_or("none of the claim's repos is on the pool's allow-list")?;
    if covered.names.len() > MOST_REPOS {
        return Err(format!(
            "the claim names {} repos, and one token covers {MOST_REPOS} at most",
            covered.names.len()
        ));
    }
    Ok(covered)
}

/// How a Secret read came out, in words that hold nothing of it.
#[derive(Debug, PartialEq)]
pub enum Read {
    Found(Box<Secret>),
    Forbidden,
    Missing,
    Failed(String),
}

/// `text`, the body the API server answered a read of a Secret with, as a Secret. A body that does
/// not parse is reported without a word of it: it may hold the key, and neither this nor anything
/// it calls logs it.
pub fn secret_of(text: &str) -> Read {
    match serde_json::from_str::<Secret>(text) {
        Ok(secret) => Read::Found(Box::new(secret)),
        Err(_) => {
            Read::Failed("the cluster answered with something other than a Secret".to_owned())
        }
    }
}

/// **The Secret `name` in the operator's namespace.** Read as text and parsed here: the client's
/// own typed read logs the whole body of an answer it cannot parse, which would be the key.
pub async fn read_secret(client: &Client, name: &str) -> Read {
    let path = format!("/api/v1/namespaces/{}/secrets", client.default_namespace());
    let request = match Request::new(path).get(name, &kube::api::GetParams::default()) {
        Ok(request) => request,
        Err(_) => return Read::Failed("the Secret's name could not be asked for".to_owned()),
    };
    let answered = tokio::time::timeout(READ_WITHIN, client.request_text(request)).await;
    match answered {
        Err(_) => Read::Failed("the cluster did not answer in time".to_owned()),
        Ok(Ok(text)) => secret_of(&text),
        Ok(Err(kube::Error::Api(status))) if status.is_forbidden() => Read::Forbidden,
        Ok(Err(kube::Error::Api(status))) if status.is_not_found() => Read::Missing,
        Ok(Err(_)) => Read::Failed("the cluster could not be asked for the Secret".to_owned()),
    }
}

/// Why `read` gives no key, in the words the log and a refusal use; or the key.
pub fn key_of(read: Read) -> Result<AppKey, String> {
    match read {
        Read::Found(secret) => AppKey::of(&secret),
        Read::Forbidden => Err(
            "the chart's forge.keySecretNames does not name it, so the operator may not read it"
                .to_owned(),
        ),
        Read::Missing => Err("it is not in the operator's namespace".to_owned()),
        Read::Failed(why) => Err(why),
    }
}

/// **Says in the log whether each opted-in Pool's key can be read**, once per thing it has to
/// say, so a Pool looked at again says nothing new. Read by name, with the operator's own right to
/// `get` what the chart names, in its own namespace.
#[derive(Default)]
struct KeyChecks {
    said: HashMap<String, Result<String, String>>,
}

impl KeyChecks {
    /// Looks at every Pool the cluster holds, and forgets what it said of any that went.
    async fn look_at(&mut self, client: &Client, pools: &[Pool]) {
        self.said
            .retain(|name, _| pools.iter().any(|pool| pool.name_any() == *name));
        for pool in pools {
            self.look_at_one(client, &pool.name_any(), &pool.spec).await;
        }
    }

    async fn look_at_one(&mut self, client: &Client, name: &str, spec: &PoolSpec) {
        let Some(secret) = key_to_read(spec) else {
            self.said.remove(name);
            return;
        };
        let read = key_of(read_secret(client, secret).await).map(|key| key.app);
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

/// **Looks at the opted-in Pools' keys every `every`**, for as long as the operator runs: a task of
/// its own, so a stalled read never holds up publishing the signing keys.
pub(crate) async fn check_keys(client: Client, every: std::time::Duration) {
    let pools = kube::Api::<Pool>::all(client.clone());
    let mut checks = KeyChecks::default();
    let mut tick = tokio::time::interval(every);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        match pools.list(&kube::api::ListParams::default()).await {
            Ok(all) => checks.look_at(&client, &all.items).await,
            Err(error) => tracing::warn!(%error, "the pools could not be listed; trying again"),
        }
    }
}
