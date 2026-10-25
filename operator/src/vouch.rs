// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The operator vouches for each runner's link key** (spec #2041, #2056).
//!
//! A runner's pod makes its link key as it starts: its first step, `purlis bridge --make-key`,
//! runs before anything else in the pod and writes the key's public half as its termination
//! message, which the cluster shows in the pod's status ([`key_published`]). The operator reads
//! it there, holds the pod to the template it made, and signs a statement of the claim, the pod,
//! the key and who asked for the claim ([`statement`]) with its own Ed25519 key
//! ([`SigningKey`]). The desktop pinned that key's public half from the Pool's status when it
//! added the pool, pairs with the runner over its own `kubectl exec`, and accepts the runner
//! only when both say the same key.
//!
//! The statement's text is the desktop's to read too (`crates/purlis-core/src/clusterpool.rs`);
//! both test against one worked example, `fixtures/runner-link-key/`.

use crate::upstream::{KEY_STEP, RUNNER_CONTAINER};
use k8s_openapi::api::core::v1::{Container, Pod, PodSpec};
use ring::signature::{Ed25519KeyPair, KeyPair};

/// The first line of every statement: what it is, and its version.
pub const STATEMENT_HEADER: &str = "purlis runner link key v1";

/// Where the operator's pod holds its signing key: a Secret the kubelet mounts there
/// (`deploy/operator.yaml`), so the operator itself holds no right to read any Secret.
pub const SIGNING_KEY_PATH: &str = "/var/run/purlis-cluster-operator/signing-key/key.pem";

/// What a statement is about, each value one plain line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct About<'a> {
    pub namespace: &'a str,
    pub claim: &'a str,
    pub claim_uid: &'a str,
    pub pod: &'a str,
    pub pod_uid: &'a str,
    /// The public link key, 64 lower-case hex.
    pub key: &'a str,
    pub requester: &'a str,
}

/// The longest value a statement's line holds.
const MOST_VALUE: usize = 512;

/// The statement's text, exactly as signed, or `None` when a value could not be one plain line:
/// empty, longer than 512 bytes, padded, or holding a control character.
pub fn statement(about: &About<'_>) -> Option<String> {
    let lines = [
        ("namespace", about.namespace),
        ("claim", about.claim),
        ("claim uid", about.claim_uid),
        ("pod", about.pod),
        ("pod uid", about.pod_uid),
        ("key", about.key),
        ("requester", about.requester),
    ];
    if !lines.iter().all(|(_, value)| plain(value)) || !is_lower_hex(about.key, 64) {
        return None;
    }
    let mut text = format!("{STATEMENT_HEADER}\n");
    for (name, value) in lines {
        text.push_str(&format!("{name}: {value}\n"));
    }
    Some(text)
}

fn plain(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MOST_VALUE
        && value.chars().all(|c| !c.is_control())
        && value.trim() == value
}

fn is_lower_hex(text: &str, len: usize) -> bool {
    text.len() == len && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The lines around a PEM `PRIVATE KEY` block, spelled in parts so that no scan of this source
/// takes them for a key.
pub const PEM_BEGIN: &str = concat!("-----BEGIN ", "PRIVATE", " KEY-----");
pub const PEM_END: &str = concat!("-----END ", "PRIVATE", " KEY-----");

/// The operator's Ed25519 signing key. Its private half never leaves this process and is
/// never printed.
pub struct SigningKey(Ed25519KeyPair);

impl std::fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SigningKey({})", self.public())
    }
}

impl SigningKey {
    /// The key a PEM `PRIVATE KEY` block holds: PKCS#8, as `openssl genpkey -algorithm ed25519`
    /// writes it.
    pub fn from_pem(pem: &str) -> Result<SigningKey, String> {
        use base64::Engine;
        let body: String = pem
            .trim()
            .strip_prefix(PEM_BEGIN)
            .and_then(|rest| rest.strip_suffix(PEM_END))
            .ok_or("the signing key is not one PEM PRIVATE KEY block")?
            .split_whitespace()
            .collect();
        let der = base64::engine::general_purpose::STANDARD
            .decode(body)
            .map_err(|_| "the signing key's PEM block is not base64")?;
        SigningKey::from_pkcs8(&der)
    }

    /// The key PKCS#8 DER `der` holds.
    pub fn from_pkcs8(der: &[u8]) -> Result<SigningKey, String> {
        Ed25519KeyPair::from_pkcs8_maybe_unchecked(der)
            .map(SigningKey)
            .map_err(|_| "the signing key is not an Ed25519 key in PKCS#8".to_owned())
    }

    /// Its public half, as a Pool's status spells it: `ed25519:` and 64 lower-case hex.
    pub fn public(&self) -> String {
        format!("ed25519:{}", hex(self.0.public_key().as_ref()))
    }

    /// Its signature over `text`, 128 lower-case hex.
    pub fn sign(&self, text: &str) -> String {
        hex(self.0.sign(text.as_bytes()).as_ref())
    }
}

/// Why a pod's key is not vouched for yet, or never will be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotYet {
    /// The pod's first step has not made the key yet.
    Starting,
    /// The pod is not one the operator's template makes for this pool: it is never vouched for.
    NotOurs(String),
}

/// **The public link key `pod` published as it started**, when it is a pod of `template`, the
/// operator's template for its pool ([`crate::upstream::template_for`]): its first step and its
/// runner are the template's two containers, each with the template's image, command,
/// arguments, environment, mounts and confinement, and its volumes are the template's. Only then
/// is what the first step published the key of the pool's image, run as the operator made it;
/// it must have ended with nothing but a key in its termination message.
pub fn key_published(pod: &Pod, template: &PodSpec) -> Result<String, NotYet> {
    let spec = pod
        .spec
        .as_ref()
        .ok_or_else(|| NotYet::NotOurs("the pod has no spec".into()))?;
    let same = |held: &[Container], wanted: &[Container]| {
        held.len() == wanted.len()
            && held.iter().zip(wanted).all(|(held, wanted)| {
                held.name == wanted.name
                    && held.image == wanted.image
                    && held.command == wanted.command
                    && held.args == wanted.args
                    && held.env == wanted.env
                    && held.env_from == wanted.env_from
                    && held.volume_mounts == wanted.volume_mounts
                    && held.volume_devices == wanted.volume_devices
                    && held.security_context == wanted.security_context
            })
    };
    let steps = spec.init_containers.as_deref().unwrap_or_default();
    let wanted_steps = template.init_containers.as_deref().unwrap_or_default();
    if !same(steps, wanted_steps) || steps.first().map(|step| step.name.as_str()) != Some(KEY_STEP)
    {
        return Err(NotYet::NotOurs(format!(
            "its first step is not the operator's {KEY_STEP} step of the pool's image"
        )));
    }
    if !same(&spec.containers, &template.containers)
        || spec.containers.first().map(|runner| runner.name.as_str()) != Some(RUNNER_CONTAINER)
    {
        return Err(NotYet::NotOurs(
            "its runner is not the operator's runner of the pool's image".into(),
        ));
    }
    if spec.volumes != template.volumes || spec.ephemeral_containers.is_some() {
        return Err(NotYet::NotOurs(
            "its volumes are not the operator's template's".into(),
        ));
    }
    let ended = pod
        .status
        .as_ref()
        .and_then(|status| status.init_container_statuses.as_ref())
        .and_then(|statuses| statuses.iter().find(|status| status.name == KEY_STEP))
        .and_then(|status| status.state.as_ref())
        .and_then(|state| state.terminated.as_ref())
        .ok_or(NotYet::Starting)?;
    if ended.exit_code != 0 {
        return Err(NotYet::NotOurs(format!(
            "the pod's {KEY_STEP} step ended with {}",
            ended.exit_code
        )));
    }
    let key = ended.message.as_deref().unwrap_or_default();
    if !is_lower_hex(key, 64) {
        return Err(NotYet::NotOurs(format!(
            "the pod's {KEY_STEP} step published no key"
        )));
    }
    Ok(key.to_owned())
}
