// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The operator mints a GitHub App token for a vouched runner** (#4206, the rulings on #3807).
//!
//! A runner of an opted-in Pool asks on [`PORT`], over TLS, in two steps, each its own connection:
//!
//! 1. `POST` [`CHALLENGE_PATH`] answers `{"challenge": <64 hex>}`: the public half of an X25519 key
//!    made for this one request, good for [`CHALLENGE_LIVES`] and for one use.
//! 2. `POST` [`TOKEN_PATH`] with `{"namespace", "claim", "app", "challenge", "proof"}` answers the
//!    token, or `{"refused": <words>}`. `proof` is what only the holder of the runner's link key
//!    can make ([`proof_for`]): a link key is an X25519 key and cannot sign, so the agreement of
//!    the link key with the challenge keys a MAC over what is asked.
//!
//! The link key checked is the one on the Claim's status, in a statement this operator signed and
//! that still holds. Then, read now, never remembered ([`may_mint`]): the Claim and its Pool were
//! made under the policy on App keys, the Pool is opted in, neither is stopped or going, and a
//! token would cover something; and the cluster is asked, before each mint, whether the requester
//! may still `use` the Pool. Only then is the App's Secret read, held to the `app` asked for, and
//! the token asked of GitHub ([`crate::github`]), limited to [`crate::github::MINTABLE`]. Each mint and refusal is in the log, with no
//! token; nothing about a token is written to the API server.
//!
//! A runner is told the port's address and the key to hold it to as it is paired, from the Claim's
//! `status.mint` ([`said_on_a_claim`]). `POST` [`NEXT_KEY_PATH`] answers the current key's word for
//! the next one during a roll, as every Pool's status holds it, so **a runner follows a roll by
//! itself**: it holds both keys till the port shows the next (#4207). TLS serves the current key
//! alone, and a proof may name either.
//!
//! **The port's certificate is self-signed by the operator's signing key** ([`certificate`]), the
//! key every Pool's status names, so a runner holds it to the key its desktop pinned, not to an
//! authority. Before a runner has proved anything, every refusal is the same words.

use crate::admission::{Admission, FORGE_POLICY, POLICY, admission, forge_in_force_since};
use crate::forge::{self, Covered};
use crate::github::GitHub;
use crate::resources::{Claim, ClaimMint, Pool};
use crate::vouch::{self, MountedKeys, SigningKey, SigningKeys};
use k8s_openapi::api::admissionregistration::v1::{
    ValidatingAdmissionPolicy, ValidatingAdmissionPolicyBinding,
};
use k8s_openapi::api::authorization::v1::{
    ResourceAttributes, SubjectAccessReview, SubjectAccessReviewSpec,
};
use k8s_openapi::api::core::v1::{Pod, Service};
use k8s_openapi::api::networking::v1::NetworkPolicy;
use k8s_openapi::jiff::Timestamp;
use kube::Client;
use kube::api::{Api, PostParams};
use ring::agreement::{self, EphemeralPrivateKey, UnparsedPublicKey, X25519};
use ring::{hkdf, hmac};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

/// The port the operator mints on, in its pod and on its Service `purlis-cluster-operator-mint`.
pub const PORT: u16 = 8443;

pub const CHALLENGE_PATH: &str = "/forge/challenge";
pub const TOKEN_PATH: &str = "/forge/token";
pub const NEXT_KEY_PATH: &str = "/forge/next-key";

/// The Service the port is reached by, in the operator's namespace.
pub const SERVICE: &str = "purlis-cluster-operator-mint";

/// How long a challenge may be answered.
pub const CHALLENGE_LIVES: Duration = Duration::from_secs(60);

/// The most challenges held: past it the oldest goes, so no number of requests spends more of the
/// operator's memory than this.
const MOST_CHALLENGES: usize = 1024;

/// The most connections held open at once; one more is closed unanswered.
const CONNECTIONS: usize = 16;

/// How long one connection may take, GitHub's answers included.
const CONNECTION_WITHIN: Duration = Duration::from_secs(60);

/// How long a request's bytes are waited for.
const REQUEST_WITHIN: Duration = Duration::from_secs(5);

/// The most a request may be, head and body.
const MOST_REQUEST: usize = 16 * 1024;

/// What an asker who has proved nothing is told, whatever the reason, so the port says nothing of
/// which claims there are. The log holds the reason.
pub const NOT_VOUCHED: &str = "the operator vouches for no such key on that claim";

/// What a runner made before its Pool named an App's key is told.
pub const NOT_MARKED: &str = "this runner was made before its pool named an App's key, so it mints \
     nothing: claim a new runner";

/// What the proof is a MAC of, each value one line. `operator` is the signing key the runner holds
/// the port to, as a Pool's status spells it, so a proof made for another key is good for nothing.
pub fn proof_text(
    operator: &str,
    namespace: &str,
    claim: &str,
    app: &str,
    challenge: &str,
) -> String {
    format!(
        "purlis forge token request v1\noperator: {operator}\nnamespace: {namespace}\nclaim: {claim}\napp: {app}\nchallenge: {challenge}\n"
    )
}

/// The MAC key `shared`, the agreement of a link key and a challenge, gives.
fn mac_key(shared: &[u8], challenge: &[u8]) -> Result<hmac::Key, String> {
    let info = [challenge];
    let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, b"purlis forge token request v1");
    let extracted = salt.extract(shared);
    let okm = extracted
        .expand(&info, hmac::HMAC_SHA256)
        .map_err(|_| "no key".to_owned())?;
    Ok(hmac::Key::from(okm))
}

/// **The proof** of holding a link key: HMAC-SHA-256 of [`proof_text`], keyed by HKDF-SHA-256 of
/// the X25519 agreement of the link key and the challenge, with the challenge as HKDF's `info`. As
/// 64 lower-case hex.
pub fn proof_for(shared: &[u8], challenge: &[u8], text: &str) -> Result<String, String> {
    let tag = hmac::sign(&mac_key(shared, challenge)?, text.as_bytes());
    Ok(hex(tag.as_ref()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2)
        || !text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).ok())
        .collect()
}

/// The challenges out, each answered once.
#[derive(Default)]
pub struct Challenges {
    out: Mutex<HashMap<String, (EphemeralPrivateKey, Instant)>>,
}

impl Challenges {
    /// A new challenge, as hex.
    pub fn make(&self) -> Result<String, String> {
        self.make_at(Instant::now())
    }

    fn make_at(&self, now: Instant) -> Result<String, String> {
        let rng = ring::rand::SystemRandom::new();
        let private = EphemeralPrivateKey::generate(&X25519, &rng)
            .map_err(|_| "no challenge could be made".to_owned())?;
        let public = private
            .compute_public_key()
            .map_err(|_| "no challenge could be made".to_owned())?;
        let challenge = hex(public.as_ref());
        let mut out = self.out.lock().unwrap_or_else(|poison| poison.into_inner());
        out.retain(|_, (_, made)| now.duration_since(*made) < CHALLENGE_LIVES);
        if out.len() >= MOST_CHALLENGES
            && let Some(oldest) = out
                .iter()
                .min_by_key(|(_, (_, made))| *made)
                .map(|(key, _)| key.clone())
        {
            out.remove(&oldest);
        }
        out.insert(challenge.clone(), (private, now));
        Ok(challenge)
    }

    /// Takes `challenge`, which never answers again, whatever comes of it.
    fn take(&self, challenge: &str, now: Instant) -> Option<EphemeralPrivateKey> {
        let mut out = self.out.lock().unwrap_or_else(|poison| poison.into_inner());
        let (private, made) = out.remove(challenge)?;
        (now.duration_since(made) < CHALLENGE_LIVES).then_some(private)
    }

    /// Whether `proof` over one of `texts` was made by the holder of the link key `key`, for `challenge`,
    /// which is used up by asking.
    pub fn proved(&self, challenge: &str, key: &[u8], texts: &[String], proof: &str) -> bool {
        self.proved_at(challenge, key, texts, proof, Instant::now())
    }

    fn proved_at(
        &self,
        challenge: &str,
        key: &[u8],
        texts: &[String],
        proof: &str,
        now: Instant,
    ) -> bool {
        let (Some(private), Some(tag), Some(challenge_bytes)) =
            (self.take(challenge, now), unhex(proof), unhex(challenge))
        else {
            return false;
        };
        let peer = UnparsedPublicKey::new(&X25519, key);
        // A low-order key agrees on nothing, which ring refuses.
        agreement::agree_ephemeral(private, &peer, |shared| {
            mac_key(shared, &challenge_bytes)
                .map(|mac| {
                    texts
                        .iter()
                        .any(|text| hmac::verify(&mac, text.as_bytes(), &tag).is_ok())
                })
                .unwrap_or(false)
        })
        .unwrap_or(false)
    }
}

/// A runner's request for a token.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Asked {
    pub namespace: String,
    pub claim: String,
    /// The GitHub App the runner's tool line names (ruling Q6).
    pub app: String,
    pub challenge: String,
    pub proof: String,
}

impl Asked {
    /// Whether each value is one the cluster or the tool line could hold, so it may be logged and
    /// put in the text the proof is of.
    pub fn well_spelled(&self) -> bool {
        vouch::is_label(&self.namespace)
            && vouch::is_subdomain(&self.claim)
            && !self.app.is_empty()
            && self.app.len() <= 64
            && self
                .app
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
            && self.challenge.len() == 64
            && self.proof.len() == 64
    }
}

/// What a statement on a Claim's status says of its runner, once it is the operator's and holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vouched {
    pub key: Vec<u8>,
    pub key_hex: String,
    pub runner_namespace: String,
    pub pod: String,
}

fn line<'a>(statement: &'a str, name: &str) -> Option<&'a str> {
    statement
        .lines()
        .find_map(|line| line.strip_prefix(name)?.strip_prefix(": "))
}

fn signed_by(keys: &SigningKeys, statement: &str, signature: &str) -> bool {
    let by = |key: &SigningKey| key.sign(statement) == signature;
    by(&keys.current) || keys.next.as_ref().is_some_and(by)
}

/// **The runner's vouched key**, from the statement on `claim`'s status: signed by this operator,
/// still holding at `now`, and of this very claim (a statement of another copied here names that
/// one's uid). Refused otherwise, in words for the log.
pub fn vouched(claim: &Claim, keys: &SigningKeys, now: u64) -> Result<Vouched, String> {
    let link_key = claim
        .status
        .as_ref()
        .and_then(|status| status.link_key.as_ref())
        .ok_or("the claim holds no statement")?;
    let statement = link_key.statement.as_str();
    if !statement.starts_with(&format!("{}\n", vouch::STATEMENT_HEADER))
        || !signed_by(keys, statement, &link_key.signature)
    {
        return Err("the claim's statement is not this operator's".to_owned());
    }
    if vouch::until_of(statement).is_none_or(|until| until <= now) {
        return Err("the claim's statement no longer holds".to_owned());
    }
    let of_this_claim = line(statement, "claim").map(str::to_owned) == claim.metadata.name
        && line(statement, "namespace").map(str::to_owned) == claim.metadata.namespace
        && line(statement, "claim uid").map(str::to_owned) == claim.metadata.uid;
    if !of_this_claim || line(statement, "requester") != Some(claim.spec.requester.as_str()) {
        return Err("the claim's statement is of another claim".to_owned());
    }
    let (Some(key_hex), Some(runner_namespace), Some(pod)) = (
        line(statement, "key"),
        line(statement, "runner namespace"),
        line(statement, "pod"),
    ) else {
        return Err("the claim's statement says too little".to_owned());
    };
    let key = unhex(key_hex)
        .filter(|key| key.len() == 32)
        .ok_or("the claim's statement names no key")?;
    Ok(Vouched {
        key,
        key_hex: key_hex.to_owned(),
        runner_namespace: runner_namespace.to_owned(),
        pod: pod.to_owned(),
    })
}

/// Whether the pod a statement names is there and still holds the key: a statement can outlive its
/// pod by minutes, and a stopped runner mints nothing.
pub fn pod_holds(pod: &Pod, key_hex: &str) -> bool {
    pod.metadata.deletion_timestamp.is_none()
        && pod
            .status
            .as_ref()
            .and_then(|status| status.phase.as_deref())
            == Some("Running")
        && pod
            .status
            .as_ref()
            .and_then(|status| status.init_container_statuses.as_ref())
            .and_then(|statuses| {
                statuses
                    .iter()
                    .find(|status| status.name == crate::upstream::KEY_STEP)
            })
            .and_then(|status| status.state.as_ref())
            .and_then(|state| state.terminated.as_ref())
            .and_then(|ended| ended.message.as_deref())
            == Some(key_hex)
}

/// Whether every container of `pod` runs `image`, its Pool's image now: a runner from before the
/// image changed mints nothing, even while its statement still holds.
pub fn runs_image(pod: &Pod, image: &str) -> bool {
    pod.spec.as_ref().is_some_and(|spec| {
        spec.containers
            .iter()
            .chain(spec.init_containers.iter().flatten())
            .all(|container| container.image.as_deref() == Some(image))
    })
}

/// **Whether `pod`'s runner was made to refuse any start outside the sandbox but a shell**
/// ([`crate::upstream::MINTING_MARK`]): only such a runner's link key has been out of a chat's reach
/// since it was made, so nothing else mints.
pub fn marked(pod: &Pod) -> bool {
    pod.spec.as_ref().is_some_and(|spec| {
        spec.containers
            .iter()
            .filter(|container| container.name == crate::upstream::RUNNER_CONTAINER)
            .flat_map(|container| container.env.iter().flatten())
            .any(|var| {
                var.name == crate::upstream::MINTING_MARK && var.value.as_deref() == Some("1")
            })
    })
}

/// What a mint may cover, and with which key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grant {
    pub covered: Covered,
    pub secret: String,
}

/// The Claim and Pool as read now, and the policies they answer to.
pub struct Held<'a> {
    pub claim: &'a Claim,
    pub pool: &'a Pool,
    /// The binding of the policy on Claims.
    pub claims_binding: Option<&'a ValidatingAdmissionPolicyBinding>,
    /// When the policy on App keys is in force from ([`forge_in_force_since`]).
    pub forge_since: Option<Timestamp>,
}

/// **Whether anything may be minted for this Claim, from its Pool and itself alone**: the desktop's
/// record of the pool has no say. Whatever was written before the policy on App keys was in force
/// was not held to it, so such a Claim or Pool never mints until it is made again.
pub fn may_mint(held: &Held<'_>) -> Result<Grant, String> {
    let Held { claim, pool, .. } = held;
    if claim.metadata.deletion_timestamp.is_some() || pool.metadata.deletion_timestamp.is_some() {
        return Err("the claim or its pool is being deleted".to_owned());
    }
    if admission(claim, held.claims_binding) != Admission::UnderPolicy {
        return Err(format!(
            "the claim was not admitted under the policy {POLICY}"
        ));
    }
    let Some(since) = held.forge_since else {
        return Err(format!("the policy {FORGE_POLICY} is not in force"));
    };
    let after = |made: &Option<k8s_openapi::apimachinery::pkg::apis::meta::v1::Time>| {
        made.as_ref().is_some_and(|made| made.0 > since)
    };
    if !after(&claim.metadata.creation_timestamp) {
        return Err(format!(
            "the claim was made before the policy {FORGE_POLICY} was in force: delete it and make it again"
        ));
    }
    if !after(&pool.metadata.creation_timestamp) {
        return Err(format!(
            "the pool was made before the policy {FORGE_POLICY} was in force: delete it and make it again"
        ));
    }
    if pool.spec.stopped || claim.spec.stopped {
        return Err("the claim or its pool is stopped".to_owned());
    }
    let Some(forge) = pool
        .spec
        .forge
        .as_ref()
        .filter(|forge| forge.mint_in_cluster)
    else {
        return Err("the pool does not opt in to minting in the cluster".to_owned());
    };
    let repos = claim
        .spec
        .forge
        .as_ref()
        .map(|forge| forge.repos.as_slice())
        .unwrap_or_default();
    Ok(Grant {
        covered: forge::covered(&forge.repos, repos)?,
        secret: forge.app_key_secret_ref.name.clone(),
    })
}

/// **The port's address**, `<ip>:<port>`, from the Service's cluster IP: a runner's pod is given
/// public name servers, so a cluster name would not resolve. None where the Service has no address
/// that is one.
pub fn address_of(service: &Service) -> Option<String> {
    let ip: std::net::IpAddr = service.spec.as_ref()?.cluster_ip.as_deref()?.parse().ok()?;
    Some(std::net::SocketAddr::new(ip, PORT).to_string())
}

/// **What a Claim's status says of the mint port** (#4207, the person's ruling of 2026-10-31):
/// only for a vouched runner on a Pool that opts in. It decides nothing: a runner holds the port to
/// the key, a desktop tells it only a key it pinned, and each mint is decided from the Claim and
/// the Pool alone.
pub fn mint_status(
    pool: &Pool,
    vouched: bool,
    address: Option<String>,
    key: &str,
) -> Option<ClaimMint> {
    let opted_in = pool
        .spec
        .forge
        .as_ref()
        .is_some_and(|forge| forge.mint_in_cluster);
    (opted_in && vouched).then_some(())?;
    Some(ClaimMint {
        address: address?,
        operator_key: key.to_owned(),
    })
}

/// [`mint_status`], the Service read now, only where it would be said. A Service that cannot be
/// read says nothing, and the next look at the Claim says it: the runner then asks its desktop.
pub async fn said_on_a_claim(
    client: &Client,
    pool: &Pool,
    vouched: bool,
    key: &str,
) -> Option<ClaimMint> {
    // Nothing is read for a Pool that would say nothing.
    mint_status(pool, vouched, Some(String::new()), key)?;
    let service = Api::<Service>::namespaced(client.clone(), client.default_namespace())
        .get_opt(SERVICE)
        .await;
    let address = match service {
        Ok(service) => service.as_ref().and_then(address_of),
        Err(why) => {
            tracing::warn!(%why, service = SERVICE, "the mint port's address could not be read");
            None
        }
    };
    mint_status(pool, vouched, address, key)
}

/// A question for the cluster's authorizer: may `requester` `use` the Pool `pool`? Asked of the
/// person by name alone, since a Claim records no groups: a right they hold only through a group
/// is not seen, which refuses rather than allows.
pub fn use_review(requester: &str, pool: &str) -> SubjectAccessReview {
    SubjectAccessReview {
        spec: SubjectAccessReviewSpec {
            user: Some(requester.to_owned()),
            resource_attributes: Some(ResourceAttributes {
                group: Some("runners.purlis.dev".to_owned()),
                resource: Some("pools".to_owned()),
                name: Some(pool.to_owned()),
                verb: Some("use".to_owned()),
                ..ResourceAttributes::default()
            }),
            ..SubjectAccessReviewSpec::default()
        },
        ..SubjectAccessReview::default()
    }
}

/// The NetworkPolicy that lets the pods of the runner namespace `namespace` reach the operator's
/// mint port, in `operator_namespace`, and nothing else. NetworkPolicies add up, so upstream's own
/// policy on them keeps every other limit.
pub fn egress_policy(namespace: &str, operator_namespace: &str) -> NetworkPolicy {
    serde_json::from_value(json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": {
            "name": forge::MINT_EGRESS_POLICY,
            "namespace": namespace,
        },
        "spec": {
            "podSelector": {},
            "policyTypes": ["Egress"],
            "egress": [{
                "to": [{
                    "namespaceSelector": {
                        "matchLabels": { "kubernetes.io/metadata.name": operator_namespace },
                    },
                    "podSelector": {
                        "matchLabels": { "app.kubernetes.io/name": "purlis-cluster-operator" },
                    },
                }],
                "ports": [{ "port": PORT, "protocol": "TCP" }],
            }],
        },
    }))
    .expect("a NetworkPolicy")
}

/// One DER value.
fn der(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let len = content.len();
    if len < 0x80 {
        out.push(len as u8);
    } else {
        let bytes: Vec<u8> = len
            .to_be_bytes()
            .into_iter()
            .skip_while(|byte| *byte == 0)
            .collect();
        out.push(0x80 | bytes.len() as u8);
        out.extend(bytes);
    }
    out.extend_from_slice(content);
    out
}

/// **The port's certificate**: X.509 v3, self-signed by `key`, naming no host and with no
/// extension. No authority stands behind it: a runner holds its public key to the operator's key
/// its desktop pinned, so a host name would add nothing, and the signing key already vouches for
/// every runner. Its dates are fixed, so one key gives one certificate.
pub fn certificate(key: &SigningKey) -> Vec<u8> {
    const SEQUENCE: u8 = 0x30;
    let ed25519 = der(SEQUENCE, &der(0x06, &[0x2b, 0x65, 0x70]));
    let name = der(
        SEQUENCE,
        &der(
            0x31,
            &der(
                SEQUENCE,
                &[
                    der(0x06, &[0x55, 0x04, 0x03]),
                    der(0x0c, b"purlis-cluster-operator"),
                ]
                .concat(),
            ),
        ),
    );
    let validity = der(
        SEQUENCE,
        &[der(0x18, b"20260101000000Z"), der(0x18, b"99991231235959Z")].concat(),
    );
    let public_key = der(
        SEQUENCE,
        &[
            ed25519.clone(),
            der(0x03, &[&[0u8][..], key.public_bytes()].concat()),
        ]
        .concat(),
    );
    let to_be_signed = der(
        SEQUENCE,
        &[
            der(0xa0, &der(0x02, &[2])),
            der(0x02, &[1]),
            ed25519.clone(),
            name.clone(),
            validity,
            name,
            public_key,
        ]
        .concat(),
    );
    let signature = key.sign_bytes(&to_be_signed);
    der(
        SEQUENCE,
        &[
            to_be_signed,
            ed25519,
            der(0x03, &[&[0u8][..], signature.as_slice()].concat()),
        ]
        .concat(),
    )
}

/// The certificate and key `key` gives TLS.
pub fn certified(key: &SigningKey) -> Result<rustls::sign::CertifiedKey, String> {
    use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
    let private = PrivatePkcs8KeyDer::from(key.pkcs8());
    let signer = rustls::crypto::ring::sign::any_eddsa_type(&private)
        .map_err(|_| "the signing key cannot serve TLS".to_owned())?;
    Ok(rustls::sign::CertifiedKey::new(
        vec![CertificateDer::from(certificate(key))],
        signer,
    ))
}

/// Hands TLS the certificate of the signing key mounted now, so a roll needs no restart.
#[derive(Debug)]
struct CurrentKey {
    keys: Arc<MountedKeys>,
    last: Mutex<Option<(String, Arc<rustls::sign::CertifiedKey>)>>,
}

impl rustls::server::ResolvesServerCert for CurrentKey {
    fn resolve(
        &self,
        _hello: rustls::server::ClientHello<'_>,
    ) -> Option<Arc<rustls::sign::CertifiedKey>> {
        let keys = self.keys.now();
        let public = keys.current.public();
        let mut last = self
            .last
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some((held, certified)) = last.as_ref()
            && *held == public
        {
            return Some(certified.clone());
        }
        let made = Arc::new(certified(&keys.current).ok()?);
        *last = Some((public, made.clone()));
        Some(made)
    }
}

/// The TLS the port serves: 1.3 alone.
fn tls(keys: Arc<MountedKeys>) -> Result<rustls::ServerConfig, String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    Ok(rustls::ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|_| "TLS 1.3 is not available".to_owned())?
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(CurrentKey {
            keys,
            last: Mutex::new(None),
        })))
}

/// A request's method, target and body. `None` for anything else: past [`MOST_REQUEST`], with a
/// body not told by one `content-length`, or cut short.
pub async fn read_request(
    stream: &mut (impl AsyncRead + Unpin),
) -> std::io::Result<Option<(String, String, Vec<u8>)>> {
    let mut read = Vec::new();
    let mut buffer = [0u8; 2048];
    let head_ends = loop {
        if let Some(at) = read.windows(4).position(|end| end == b"\r\n\r\n") {
            break at + 4;
        }
        if read.len() >= MOST_REQUEST {
            return Ok(None);
        }
        let n = stream.read(&mut buffer).await?;
        if n == 0 {
            return Ok(None);
        }
        read.extend_from_slice(&buffer[..n]);
    };
    let head = String::from_utf8_lossy(&read[..head_ends]).into_owned();
    let mut lines = head.split("\r\n");
    let mut words = lines.next().unwrap_or_default().split(' ');
    let (Some(method), Some(target)) = (words.next(), words.next()) else {
        return Ok(None);
    };
    let (mut length, mut lengths) = (0usize, 0);
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Ok(None);
        }
        if name.eq_ignore_ascii_case("content-length") {
            lengths += 1;
            match value.trim().parse() {
                Ok(told) => length = told,
                Err(_) => return Ok(None),
            }
        }
    }
    if lengths > 1 || head_ends.saturating_add(length) > MOST_REQUEST {
        return Ok(None);
    }
    while read.len() < head_ends + length {
        let n = stream.read(&mut buffer).await?;
        if n == 0 {
            return Ok(None);
        }
        read.extend_from_slice(&buffer[..n]);
    }
    let body = read[head_ends..head_ends + length].to_vec();
    Ok(Some((method.to_owned(), target.to_owned(), body)))
}

/// An answer, closing the connection.
pub fn answer(status: &str, body: &serde_json::Value) -> String {
    let body = body.to_string();
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\ncache-control: no-store\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

const REFUSED: &str = "403 Forbidden";

fn refused(why: &str) -> String {
    answer(REFUSED, &json!({ "refused": why }))
}

struct Serving {
    client: Client,
    keys: Arc<MountedKeys>,
    challenges: Challenges,
    github: Result<GitHub, String>,
    cluster: String,
}

fn seconds_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default()
}

impl Serving {
    async fn claim_and_proof(&self, asked: &Asked) -> Result<(Claim, Vouched), String> {
        let claim = Api::<Claim>::namespaced(self.client.clone(), &asked.namespace)
            .get_opt(&asked.claim)
            .await
            .map_err(|_| "the claim could not be read".to_owned())?
            .ok_or("no such claim")?;
        let keys = self.keys.now();
        let runner = vouched(&claim, &keys, seconds_now())?;
        // Either key during a roll: the runner holds the port to the one its desktop pinned.
        let texts: Vec<String> = std::iter::once(&keys.current)
            .chain(keys.next.as_ref())
            .map(|key| {
                proof_text(
                    &key.public(),
                    &asked.namespace,
                    &asked.claim,
                    &asked.app,
                    &asked.challenge,
                )
            })
            .collect();
        if !self
            .challenges
            .proved(&asked.challenge, &runner.key, &texts, &asked.proof)
        {
            return Err("the proof is not of the claim's vouched key".to_owned());
        }
        Ok((claim, runner))
    }

    /// Everything after the runner proved its key; a refusal here is told to it in words.
    async fn mint(&self, asked: &Asked, claim: &Claim, runner: &Vouched) -> Result<String, String> {
        let client = &self.client;
        let read_failed = |what: &str| format!("the {what} could not be read");
        let pod = Api::<Pod>::namespaced(client.clone(), &runner.runner_namespace)
            .get_opt(&runner.pod)
            .await
            .map_err(|_| read_failed("runner's pod"))?;
        let Some(pod) = pod.filter(|pod| pod_holds(pod, &runner.key_hex)) else {
            return Err("the runner's pod is gone, or no longer holds this key".to_owned());
        };
        if !marked(&pod) {
            return Err(NOT_MARKED.to_owned());
        }
        let pool = Api::<Pool>::all(client.clone())
            .get_opt(&claim.spec.pool)
            .await
            .map_err(|_| read_failed("pool"))?
            .ok_or("the claim's pool is gone")?;
        if !runs_image(&pod, &pool.spec.image) {
            return Err("the runner's pod does not run its pool's image".to_owned());
        }
        let bindings = Api::<ValidatingAdmissionPolicyBinding>::all(client.clone());
        let claims_binding = bindings
            .get_opt(POLICY)
            .await
            .map_err(|_| read_failed("policy's binding"))?;
        let forge_binding = bindings
            .get_opt(FORGE_POLICY)
            .await
            .map_err(|_| read_failed("policy's binding"))?;
        let forge_policy = Api::<ValidatingAdmissionPolicy>::all(client.clone())
            .get_opt(FORGE_POLICY)
            .await
            .map_err(|_| read_failed("policy"))?;
        let grant = may_mint(&Held {
            claim,
            pool: &pool,
            claims_binding: claims_binding.as_ref(),
            forge_since: forge_in_force_since(forge_policy.as_ref(), forge_binding.as_ref()),
        })?;
        // Before each mint (ruled on #4206), so a revoked right stops new tokens at once.
        let review = Api::<SubjectAccessReview>::all(client.clone())
            .create(
                &PostParams::default(),
                &use_review(&claim.spec.requester, &claim.spec.pool),
            )
            .await
            .map_err(|_| "the cluster could not be asked who may use the pool".to_owned())?;
        let status = review.status.as_ref();
        if !status.is_some_and(|status| status.allowed && status.denied != Some(true)) {
            return Err(format!(
                "{} may not use the pool {}, by name",
                claim.spec.requester, claim.spec.pool
            ));
        }
        let key = forge::key_of(forge::read_secret(client, &grant.secret).await)
            .map_err(|why| format!("the pool's App key {}: {why}", grant.secret))?;
        key.serves(&asked.app)?;
        let github = self.github.as_ref().map_err(|why| why.clone())?;
        let jwt = key.jwt(seconds_now())?;
        let Covered { owner, names } = &grant.covered;
        let first = names.first().ok_or("nothing to cover")?;
        let installation = github.installation(&jwt, owner, first).await?;
        let permissions = crate::github::mintable(&installation.permissions);
        if permissions.is_empty() {
            return Err("the installation holds no permission a token may ask for".to_owned());
        }
        let token = github
            .token(&jwt, installation.id, names, &permissions)
            .await?;
        tracing::info!(
            namespace = %asked.namespace, claim = %asked.claim, pool = %claim.spec.pool,
            requester = %claim.spec.requester, app = %asked.app, secret = %grant.secret,
            installation = installation.id, %owner, repos = ?names,
            permissions = ?permissions.keys().collect::<Vec<_>>(),
            expires_at = ?token.expires_at,
            "minted a token"
        );
        Ok(answer(
            "200 OK",
            &json!({
                "token": token.token,
                "expires_at": token.expires_at,
                "owner": owner,
                "repos": names,
                "permissions": permissions,
            }),
        ))
    }

    async fn token(&self, body: &[u8]) -> String {
        let Some(asked) = serde_json::from_slice::<Asked>(body)
            .ok()
            .filter(Asked::well_spelled)
        else {
            tracing::warn!(why = "the request cannot be read", "refused a token");
            return refused(NOT_VOUCHED);
        };
        let (claim, runner) = match self.claim_and_proof(&asked).await {
            Ok(proved) => proved,
            Err(why) => {
                tracing::warn!(namespace = %asked.namespace, claim = %asked.claim, app = %asked.app, %why, "refused a token");
                return refused(NOT_VOUCHED);
            }
        };
        match self.mint(&asked, &claim, &runner).await {
            Ok(answered) => answered,
            Err(why) => {
                tracing::warn!(
                    namespace = %asked.namespace, claim = %asked.claim, pool = %claim.spec.pool,
                    requester = %claim.spec.requester, app = %asked.app, %why, "refused a token"
                );
                refused(&why)
            }
        }
    }

    /// During a roll, the next key with the current key's word for it, as every Pool's status says
    /// them; otherwise nothing. Nobody need prove anything: none of it is a secret.
    fn next_key(&self) -> String {
        let status = vouch::pool_status(&self.keys.now(), &self.cluster, None, seconds_now());
        match (status.next_signing_key, status.next_signing_key_vouch) {
            (Some(next), Some(word)) => answer(
                "200 OK",
                &json!({
                    "next": next,
                    "statement": word.statement,
                    "signature": word.signature,
                }),
            ),
            _ => answer("200 OK", &json!({})),
        }
    }

    async fn respond(&self, method: &str, target: &str, body: &[u8]) -> String {
        match (method, target) {
            ("POST", NEXT_KEY_PATH) => self.next_key(),
            ("POST", CHALLENGE_PATH) => match self.challenges.make() {
                Ok(challenge) => answer("200 OK", &json!({ "challenge": challenge })),
                Err(why) => answer("500 Internal Server Error", &json!({ "refused": why })),
            },
            ("POST", TOKEN_PATH) => self.token(body).await,
            _ => answer("404 Not Found", &json!({ "refused": "no such request" })),
        }
    }
}

async fn serve_one(
    stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
    serving: &Serving,
) -> std::io::Result<()> {
    let read = tokio::time::timeout(REQUEST_WITHIN, read_request(stream)).await;
    let answered = match read {
        Ok(Ok(Some((method, target, body)))) => serving.respond(&method, &target, &body).await,
        Ok(Err(why)) => return Err(why),
        _ => {
            tracing::warn!(why = "no request could be read", "refused a token");
            answer("400 Bad Request", &json!({ "refused": "no request read" }))
        }
    };
    stream.write_all(answered.as_bytes()).await?;
    stream.shutdown().await
}

/// **Serves the mint port** until the process ends. A pod with no Pool opted in refuses every
/// token, and where the cluster enforces NetworkPolicies only runner namespaces
/// reach it. Never returns, so a port that cannot be served never stops the rest of the operator.
pub async fn serve(client: Client, keys: Arc<MountedKeys>, cluster: String) {
    if let Err(why) = serve_until_broken(client, keys, cluster).await {
        tracing::error!(%why, port = PORT, "no token will be minted: the mint port is not served");
    }
    std::future::pending::<()>().await;
}

async fn serve_until_broken(
    client: Client,
    keys: Arc<MountedKeys>,
    cluster: String,
) -> Result<(), String> {
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(tls(keys.clone())?));
    let bound = match TcpListener::bind(("::", PORT)).await {
        Ok(listener) => Ok(listener),
        Err(_) => TcpListener::bind(("0.0.0.0", PORT)).await,
    };
    let listener = bound.map_err(|why| why.to_string())?;
    let github = GitHub::new();
    if let Err(why) = &github {
        tracing::error!(%why, "no token will be minted");
    }
    let serving = Arc::new(Serving {
        client,
        keys,
        challenges: Challenges::default(),
        github,
        cluster,
    });
    tracing::info!(port = PORT, "serving the mint port");
    let open = Arc::new(Semaphore::new(CONNECTIONS));
    let mut said_full = None;
    loop {
        let stream = match listener.accept().await {
            Ok((stream, _)) => stream,
            Err(why) => {
                tracing::warn!(%why, "a connection to the mint port was not accepted");
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
        };
        let Ok(permit) = open.clone().try_acquire_owned() else {
            // Said at most once a minute, so a flood of connections is not a flood of log.
            if said_full.is_none_or(|said: Instant| said.elapsed() >= Duration::from_secs(60)) {
                said_full = Some(Instant::now());
                tracing::warn!(
                    most = CONNECTIONS,
                    "the mint port is full: connections are being dropped"
                );
            }
            continue;
        };
        let (acceptor, serving) = (acceptor.clone(), serving.clone());
        tokio::spawn(async move {
            let _permit = permit;
            let served = tokio::time::timeout(CONNECTION_WITHIN, async {
                let mut stream = tokio::time::timeout(REQUEST_WITHIN, acceptor.accept(stream))
                    .await
                    .map_err(std::io::Error::other)??;
                serve_one(&mut stream, &serving).await
            })
            .await;
            if let Ok(Err(why)) = served {
                tracing::debug!(%why, "a connection to the mint port ended");
            }
        });
    }
}
