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

use crate::reconcile::FIELD_MANAGER;
use crate::resources::{Claim, KEY_PUBLISHED, LinkKey, condition_of};
use crate::upstream::{KEY_STEP, SandboxClaim, TERMINATION_LOG};
use k8s_openapi::api::core::v1::{Pod, PodSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, ObjectMeta};
use kube::ResourceExt;
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

/// The statement's text, exactly as signed, or `None` when a value could not be one plain line
/// (empty, longer than 512 bytes, padded, or holding a control character), a name is not one
/// Kubernetes gives (a DNS-1123 label for the namespace, a subdomain for the claim and the pod),
/// or a uid is not one.
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
    let named = is_label(about.namespace)
        && is_subdomain(about.claim)
        && is_subdomain(about.pod)
        && is_uid(about.claim_uid)
        && is_uid(about.pod_uid);
    if !named || !lines.iter().all(|(_, value)| plain(value)) || !is_lower_hex(about.key, 64) {
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

/// A DNS-1123 label, as a namespace is named.
pub fn is_label(name: &str) -> bool {
    is_subdomain(name) && name.len() <= 63 && !name.contains('.')
}

/// A DNS-1123 subdomain, as a pod or a claim is named.
pub fn is_subdomain(name: &str) -> bool {
    let plain = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    !name.is_empty()
        && name.len() <= 253
        && name.split('.').all(|part| {
            !part.is_empty()
                && part.bytes().all(|b| plain(b) || b == b'-')
                && part.bytes().next().is_some_and(plain)
                && part.bytes().last().is_some_and(plain)
        })
}

/// A uid as Kubernetes makes one: a UUID in lower-case hex, `8-4-4-4-12`.
pub fn is_uid(uid: &str) -> bool {
    let parts: Vec<&str> = uid.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(part, len)| is_lower_hex(part, len))
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
    /// The pod is not the one the operator's template makes for this claim: its key is never
    /// vouched for, and why.
    Refused(String),
}

/// The fields of a pod's spec the API server, its admission or the scheduler set or default,
/// which say nothing of what runs in it: each is left out of both sides before the pod is
/// compared with the template ([`normalised`]). Every other field must be the template's.
pub const SET_BY_THE_CLUSTER: &[&str] = &[
    "nodeName (any)",
    "schedulerName (any)",
    "tolerations (only the not-ready and unreachable ones added for 300 seconds)",
    "dnsPolicy (ClusterFirst), or dnsPolicy None with exactly the name servers the pinned upstream gives a pod whose network it manages",
    "restartPolicy (Always)",
    "terminationGracePeriodSeconds (30)",
    "priority (0)",
    "preemptionPolicy (PreemptLowerPriority)",
    "serviceAccountName and serviceAccount (default)",
    "securityContext (empty)",
    "a container's imagePullPolicy (any)",
    "a container's resources (any)",
    "a container's terminationMessagePath (/dev/termination-log)",
    "a container's terminationMessagePolicy (File)",
];

/// `spec` as JSON, with each field of [`SET_BY_THE_CLUSTER`] left out where it holds what the
/// cluster sets.
pub fn normalised(spec: &PodSpec) -> serde_json::Value {
    use serde_json::{Value, json};
    let mut value = serde_json::to_value(spec).unwrap_or(Value::Null);
    let Some(pod) = value.as_object_mut() else {
        return value;
    };
    let drop_if = |pod: &mut serde_json::Map<String, Value>, field: &str, default: Value| {
        if pod.get(field) == Some(&default) {
            pod.remove(field);
        }
    };
    pod.remove("nodeName");
    pod.remove("schedulerName");
    drop_if(pod, "dnsPolicy", json!("ClusterFirst"));
    let upstreams_dns = json!({ "nameservers": crate::upstream::MANAGED_NAME_SERVERS });
    if pod.get("dnsPolicy") == Some(&json!("None")) && pod.get("dnsConfig") == Some(&upstreams_dns)
    {
        pod.remove("dnsPolicy");
        pod.remove("dnsConfig");
    }
    drop_if(pod, "restartPolicy", json!("Always"));
    drop_if(pod, "terminationGracePeriodSeconds", json!(30));
    drop_if(pod, "priority", json!(0));
    drop_if(pod, "preemptionPolicy", json!("PreemptLowerPriority"));
    drop_if(pod, "serviceAccountName", json!("default"));
    drop_if(pod, "serviceAccount", json!("default"));
    drop_if(pod, "securityContext", json!({}));
    if let Some(Value::Array(tolerations)) = pod.get_mut("tolerations") {
        tolerations.retain(|toleration| {
            ![
                "node.kubernetes.io/not-ready",
                "node.kubernetes.io/unreachable",
            ]
            .iter()
            .any(|key| {
                *toleration
                    == json!({
                        "key": key,
                        "operator": "Exists",
                        "effect": "NoExecute",
                        "tolerationSeconds": 300,
                    })
            })
        });
    }
    drop_if(pod, "tolerations", json!([]));
    for list in ["initContainers", "containers"] {
        if let Some(Value::Array(containers)) = pod.get_mut(list) {
            for container in containers.iter_mut().filter_map(Value::as_object_mut) {
                container.remove("imagePullPolicy");
                container.remove("resources");
                drop_if(container, "terminationMessagePath", json!(TERMINATION_LOG));
                drop_if(container, "terminationMessagePolicy", json!("File"));
            }
        }
    }
    value
}

/// The top-level fields in which `held` and `wanted`, each [`normalised`], differ: what a
/// refusal names, never their values.
fn differing(held: &serde_json::Value, wanted: &serde_json::Value) -> Vec<String> {
    let empty = serde_json::Map::new();
    let held = held.as_object().unwrap_or(&empty);
    let wanted = wanted.as_object().unwrap_or(&empty);
    let mut fields: Vec<String> = held
        .keys()
        .chain(wanted.keys())
        .filter(|field| held.get(*field) != wanted.get(*field))
        .cloned()
        .collect();
    fields.sort();
    fields.dedup();
    fields
}

/// **The public link key `pod` published as it started**, when `pod`'s spec is `template`'s,
/// the operator's template for its pool ([`crate::upstream::template_for`]), in every field but
/// those the cluster sets ([`SET_BY_THE_CLUSTER`]): so its first step ran the pool's image as
/// the operator made it, before anything else in the pod. It must have ended with nothing but a
/// key in its termination message.
pub fn key_published(pod: &Pod, template: &PodSpec) -> Result<String, NotYet> {
    let spec = pod
        .spec
        .as_ref()
        .ok_or_else(|| NotYet::Refused("it has no spec".into()))?;
    let differ = differing(&normalised(spec), &normalised(template));
    if !differ.is_empty() {
        return Err(NotYet::Refused(format!(
            "it is not the operator's template for its pool in {}",
            differ.join(", ")
        )));
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
        return Err(NotYet::Refused(format!(
            "its {KEY_STEP} step ended with {}",
            ended.exit_code
        )));
    }
    let key = ended.message.as_deref().unwrap_or_default();
    if !is_lower_hex(key, 64) {
        return Err(NotYet::Refused(format!(
            "its {KEY_STEP} step published no key"
        )));
    }
    Ok(key.to_owned())
}

/// Whether `object` is controlled by the object `owner` of `kind` in the API group `group`, by
/// its uid: what an owner reference anyone can write cannot fake without the owner's uid, and
/// what names alone never prove.
pub fn controlled_by(object: &ObjectMeta, group: &str, kind: &str, owner: &ObjectMeta) -> bool {
    let Some(uid) = owner.uid.as_deref().filter(|uid| !uid.is_empty()) else {
        return false;
    };
    object.owner_references.iter().flatten().any(|reference| {
        reference.controller == Some(true)
            && reference.kind == kind
            && reference.uid == uid
            && Some(reference.name.as_str()) == owner.name.as_deref()
            && reference
                .api_version
                .split_once('/')
                .is_some_and(|(of, _)| of == group)
    })
}

/// Whether some manager other than the operator owns a field of `template`'s spec: a template
/// someone else changed, which the operator's own apply would not take back.
pub fn changed_by_another(template: &ObjectMeta) -> bool {
    template.managed_fields.iter().flatten().any(|entry| {
        entry.manager.as_deref() != Some(FIELD_MANAGER)
            && entry
                .fields_v1
                .as_ref()
                .is_some_and(|fields| fields.0.get("f:spec").is_some())
    })
}

/// The reasons of a Claim's `KeyPublished` condition.
pub const STARTING: &str = "Starting";
pub const VOUCHED: &str = "Vouched";
pub const NOT_VOUCHED: &str = "NotVouched";

/// **The operator's word on a claim's runner's link key.**
#[derive(Clone, Debug, PartialEq)]
pub enum Vouching {
    /// The runner's pod, or its key, is not there yet: looked at again soon.
    Starting(String),
    /// The statement, signed.
    Vouched(LinkKey),
    /// The operator vouches for no key of this claim's runner, and why.
    Refused(String),
}

/// How soon a claim whose runner has not published its key yet is looked at again.
pub const KEY_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

/// How often a claim the operator vouched for, or refused, is looked at again, so a pod changed
/// or replaced since is found without a watch on pods.
pub const RECHECK: std::time::Duration = std::time::Duration::from_secs(60);

impl Vouching {
    /// The Claim's `KeyPublished` condition.
    pub fn condition(&self) -> Condition {
        match self {
            Vouching::Starting(why) => condition_of(KEY_PUBLISHED, "False", STARTING, why.clone()),
            Vouching::Vouched(_) => condition_of(
                KEY_PUBLISHED,
                "True",
                VOUCHED,
                "The cluster operator vouches for the link key the runner's pod made as it started."
                    .to_owned(),
            ),
            Vouching::Refused(why) => condition_of(
                KEY_PUBLISHED,
                "False",
                NOT_VOUCHED,
                format!("The cluster operator vouches for no key of this claim's runner: {why}."),
            ),
        }
    }

    /// The statement on the Claim's status: none unless vouched, so a statement once written
    /// goes as soon as it no longer holds.
    pub fn link_key(&self) -> Option<LinkKey> {
        match self {
            Vouching::Vouched(link_key) => Some(link_key.clone()),
            _ => None,
        }
    }

    /// When the claim is looked at again.
    pub fn again(&self) -> std::time::Duration {
        match self {
            Vouching::Starting(_) => KEY_WAIT,
            _ => RECHECK,
        }
    }
}

/// What the cluster holds for one claim's runner, as the operator read it.
pub struct Held<'a> {
    pub claim: &'a Claim,
    /// The claim's upstream claim, the operator's own.
    pub upstream: &'a SandboxClaim,
    /// The pool's template in the claim's namespace, as the cluster holds it.
    pub template: Option<&'a ObjectMeta>,
    /// The sandbox the upstream claim's status names.
    pub sandbox: Option<&'a ObjectMeta>,
    /// The pod of that sandbox's name.
    pub pod: Option<&'a Pod>,
}

/// **Vouches for a claim's runner's link key, or says why not** (#2056): the chain from the
/// claim to its pod by uid (upstream claim, its sandbox, its pod, each controlling the next),
/// the pool's template changed by no one else, the pod the operator's `template` in full, and
/// the key its first step published; then the statement, signed with `signing`.
pub fn vouch(held: &Held<'_>, template: &PodSpec, signing: &SigningKey) -> Vouching {
    let not_yet = |why: &str| Vouching::Starting(why.to_owned());
    let Some(named) = held
        .upstream
        .status
        .as_ref()
        .and_then(|status| status.sandbox.as_ref())
    else {
        return not_yet("The runner's pod has not been made yet.");
    };
    let Some(sandbox) = held
        .sandbox
        .filter(|sandbox| sandbox.name.as_deref() == Some(named.name.as_str()))
    else {
        return not_yet("The runner's pod has not been made yet.");
    };
    if !controlled_by(
        sandbox,
        "extensions.agents.x-k8s.io",
        "SandboxClaim",
        &held.upstream.metadata,
    ) {
        return Vouching::Refused(
            "the sandbox its upstream claim names is not that claim's".into(),
        );
    }
    if held.template.is_some_and(changed_by_another) {
        return Vouching::Refused(
            "its pool's template was changed by someone other than the operator".into(),
        );
    }
    let Some(pod) = held.pod else {
        return not_yet("The runner's pod has not been made yet.");
    };
    if !controlled_by(&pod.metadata, "agents.x-k8s.io", "Sandbox", sandbox) {
        return Vouching::Refused("its pod is not the one its sandbox made".into());
    }
    let key = match key_published(pod, template) {
        Ok(key) => key,
        Err(NotYet::Starting) => return not_yet("The runner's pod has not made its link key yet."),
        Err(NotYet::Refused(why)) => return Vouching::Refused(why),
    };
    let claim = held.claim;
    let (claim_name, claim_uid) = (claim.name_any(), claim.uid().unwrap_or_default());
    let (pod_name, pod_uid) = (pod.name_any(), pod.uid().unwrap_or_default());
    let namespace = claim.namespace().unwrap_or_default();
    let about = About {
        namespace: &namespace,
        claim: &claim_name,
        claim_uid: &claim_uid,
        pod: &pod_name,
        pod_uid: &pod_uid,
        key: &key,
        requester: &claim.spec.requester,
    };
    match statement(&about) {
        Some(text) => Vouching::Vouched(LinkKey {
            signature: signing.sign(&text),
            statement: text,
        }),
        None => {
            Vouching::Refused("its requester or its names cannot be written in a statement".into())
        }
    }
}
