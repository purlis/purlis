// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner in a team's cluster, and who vouches for it** (spec #2041, #2056): a Claim on a
//! Pool of purlis's cluster operator (`operator/`, `docs/cluster-operator.md`) gets a pod running
//! the runner image, reached through `kubectl exec -i` (ADR 0078 §2) and linked like any runner.
//!
//! **Two witnesses say who the runner is, and both must agree** (ADR 0078, amended 2026-10-26):
//!
//! - **The runner makes its link key as its pod starts**: the pod's first step, before anything
//!   else in it runs, makes the key, hands its private half to the runner's host and writes its
//!   public half where the cluster shows it in the pod's status.
//! - **The operator signs what it saw** ([`Vouched`]): the claim, the pod, that public key and
//!   who asked for the claim, signed with the operator's own Ed25519 key, on the Claim's status.
//!   The desktop pinned that key and the pool's cluster when the pool was added ([`PoolPin`]),
//!   from the Pool's status, so a statement signed by any other key, or made in another
//!   cluster, is refused.
//! - **The desktop pairs over its own `kubectl exec`** ([`pair`]): the runner answers with its
//!   device id and public key, as a Docker pool's does. A key that is not the one the operator
//!   vouched for is refused ([`Refused::KeyChanged`]), and so is a statement that no longer
//!   names the pod and key the desktop paired with, before every link ([`still_vouched`]).
//!
//! **A claim is kept or fresh** (#2057), and purlis chooses ([`kept_for`]): a kept claim's runner
//! has a volume of its own for its home, so its pod made again keeps its work and its pairing,
//! which belongs to the claim and not to the pod (ruled 2026-10-28); a fresh claim's comes warm
//! from the pool and keeps nothing.
//!
//! **A team admin rolls the operator's key** (#3305): during a roll the Pool's status names the
//! next key with the current key's word for it ([`KeyRoll`]), and every statement is signed by
//! both. A desktop that sees the roll moves its pin to the next key without asking anyone
//! ([`PoolPin::follow`]); one that missed it asks again, as for any key it did not pin.

use std::path::PathBuf;

use purlis_session_protocol::hostlink::{Pinned, PublicKey};

use crate::runnerlink::{bytes_of, hex_of, is_lower_hex};

/// Why a cluster runner was refused, or could not be reached, in words a person reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The runner answered with another key than the one the operator vouched for, or the
    /// operator now vouches for another pod or key than the one this desktop paired with: it is
    /// refused, and the person is asked whether to claim a new runner.
    KeyChanged { claim: String },
    /// The cluster pool's Pool now names another cluster operator key than the one this desktop
    /// pinned when it added the pool (#3292): every runner of it is refused, and the person is
    /// asked about it, as for a changed runner key.
    OperatorKeyChanged { pool: String },
    /// No statement this desktop can trust vouches for the runner: none yet, one signed by
    /// another key than the pool's, or one about another claim or requester.
    NotVouched(String),
    /// The whole wait for the operator to vouch passed with no statement yet (spec #4275, #4398):
    /// the one refusal a longer wait could have changed.
    WaitRanOut(String),
    /// Anything else, in words a person reads.
    Other(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::KeyChanged { claim } => write!(
                f,
                "the runner of claim {claim} answered with another key than the one the \
                 cluster operator vouched for, so purlis refused it; delete the claim and \
                 claim a new runner if you expected this"
            ),
            Refused::OperatorKeyChanged { pool } => write!(
                f,
                "the cluster's pool {pool} now names another cluster operator key than the one \
                 this desktop pinned when the pool was added, so purlis refused its runner; \
                 check the new key with your cluster's admin, then remove the pool and add it \
                 again"
            ),
            Refused::NotVouched(why) | Refused::WaitRanOut(why) | Refused::Other(why) => {
                f.write_str(why)
            }
        }
    }
}

impl std::error::Error for Refused {}

/// The first line of every statement: what it is, and its version.
pub const STATEMENT_HEADER: &str = "purlis runner link key v3";

/// **What the cluster operator vouches for**: one claim's runner, the pod it runs in, by name,
/// in its requester's runner namespace (#2055), the public link key that runner holds, and who
/// asked for the claim. A kept claim's pod made
/// again has the same name and key (#2057), so the statement says the same. Signed as its
/// text ([`Vouched::text`]), one `name: value` line each, in this order, after
/// [`STATEMENT_HEADER`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vouched {
    /// The cluster it was made in: the uid of that cluster's `kube-system` namespace (#3305).
    pub cluster: String,
    /// The claim's namespace.
    pub namespace: String,
    pub claim: String,
    pub claim_uid: String,
    /// The namespace the runner's pod is in: its requester's own ([`runner_namespace`]).
    pub runner_namespace: String,
    pub pod: String,
    pub key: PublicKey,
    pub requester: String,
    /// When it stops holding, in seconds since the epoch (#2057): a desktop refuses it after,
    /// and the operator signs it again well before, as long as it still vouches.
    pub until: u64,
}

/// The names of a statement's lines, in their order.
const LINES: [&str; 9] = [
    "cluster",
    "namespace",
    "claim",
    "claim uid",
    "runner namespace",
    "pod",
    "key",
    "requester",
    "until",
];

/// The furthest ahead a statement's `until` may be: an operator signs one for ten minutes, and
/// purlis allows five more for this machine's clock running behind the cluster's, and reads none
/// that claims to hold past that, whatever signed it.
pub const MOST_AHEAD: u64 = 15 * 60;

/// The longest value a statement's line holds.
const MOST_VALUE: usize = 512;

impl Vouched {
    /// Its text, exactly as the operator signs it.
    pub fn text(&self) -> String {
        let key = hex_of(self.key.as_bytes());
        let until = self.until.to_string();
        let values = [
            self.cluster.as_str(),
            &self.namespace,
            &self.claim,
            &self.claim_uid,
            &self.runner_namespace,
            &self.pod,
            &key,
            &self.requester,
            &until,
        ];
        let mut text = format!("{STATEMENT_HEADER}\n");
        for (name, value) in LINES.iter().zip(values) {
            text.push_str(&format!("{name}: {value}\n"));
        }
        text
    }

    /// A statement from its text: exactly [`Vouched::text`]'s lines, each value plain printable
    /// text, and its key 64 lower-case hex. Anything else is refused.
    pub fn from_text(text: &str) -> Result<Vouched, Refused> {
        let bad = || {
            Refused::NotVouched("the cluster operator's statement is not one purlis reads".into())
        };
        if let Some(version) = text
            .lines()
            .next()
            .and_then(|first| first.strip_prefix("purlis runner link key "))
            .filter(|version| text.lines().next() != Some(STATEMENT_HEADER) && plain(version))
        {
            return Err(Refused::NotVouched(format!(
                "the cluster operator wrote a statement of version {version}, and this purlis \
                 reads {}: update purlis or the cluster operator so both are of one release",
                &STATEMENT_HEADER["purlis runner link key ".len()..]
            )));
        }
        let body = text
            .strip_prefix(STATEMENT_HEADER)
            .and_then(|rest| rest.strip_prefix('\n'))
            .ok_or_else(bad)?;
        let body = body.strip_suffix('\n').ok_or_else(bad)?;
        let lines: Vec<&str> = body.split('\n').collect();
        if lines.len() != LINES.len() {
            return Err(bad());
        }
        let mut values = Vec::with_capacity(LINES.len());
        for (line, name) in lines.iter().zip(LINES) {
            let value = line
                .strip_prefix(name)
                .and_then(|rest| rest.strip_prefix(": "))
                .filter(|value| plain(value))
                .ok_or_else(bad)?;
            values.push(value.to_owned());
        }
        let key = bytes_of(&values[6]).ok_or_else(bad)?;
        let [
            cluster,
            namespace,
            claim,
            claim_uid,
            runner_namespace,
            pod,
            _key,
            requester,
            until,
        ]: [String; 9] = values.try_into().map_err(|_| bad())?;
        let until = Some(until)
            .filter(|digits| digits.len() <= 20 && digits.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|digits| digits.parse::<u64>().ok())
            .ok_or_else(bad)?;
        // Names and uids only as Kubernetes gives them: nothing that could read as a flag or
        // another kind of object where kubectl is handed the pod's name.
        let named = is_label(&namespace)
            && is_label(&runner_namespace)
            && is_subdomain(&claim)
            && is_subdomain(&pod)
            && is_uid(&claim_uid)
            && is_uid(&cluster);
        if !named {
            return Err(bad());
        }
        Ok(Vouched {
            cluster,
            namespace,
            claim,
            claim_uid,
            runner_namespace,
            pod,
            key: PublicKey::from_bytes(key),
            requester,
            until,
        })
    }

    /// Whether it is about the claim `namespace`/`claim` of uid `claim_uid`, asked for by
    /// `requester`, with its runner in that requester's own runner namespace: the one this
    /// desktop made.
    pub fn is_for(&self, namespace: &str, claim: &str, claim_uid: &str, requester: &str) -> bool {
        self.namespace == namespace
            && self.claim == claim
            && self.claim_uid == claim_uid
            && self.requester == requester
            && self.runner_namespace == runner_namespace(requester)
    }
}

/// **The runner namespace of `requester`** (#2055): `purlis-runners-` and the first 16 lower-case
/// hex digits of the SHA-256 of the requester, exactly as the Claim names them. The cluster
/// operator makes it and runs every runner of that person's claims there; a statement naming any
/// other is refused.
pub fn runner_namespace(requester: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(requester.as_bytes());
    format!("purlis-runners-{}", &hex_of(digest.as_slice())[..16])
}

/// Whether `value` can be a statement's value: not empty, at most [`MOST_VALUE`] bytes, and
/// nothing a line break or a control character could hide behind.
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

fn plain(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MOST_VALUE
        && value.chars().all(|c| !c.is_control())
        && value.trim() == value
}

/// **The cluster operator's public Ed25519 key**, which a desktop pins when it adds the pool,
/// spelled `ed25519:<64 lower-case hex>` as the Pool's status gives it.
#[derive(Clone, PartialEq, Eq)]
pub struct OperatorKey([u8; 32]);

/// The prefix of an operator key's spelling.
const ED25519: &str = "ed25519:";

impl OperatorKey {
    /// The key `spelled` names, or a refusal saying it is not one.
    pub fn from_spelled(spelled: &str) -> Result<OperatorKey, Refused> {
        spelled
            .strip_prefix(ED25519)
            .and_then(bytes_of)
            .map(OperatorKey)
            .ok_or_else(|| {
                Refused::Other(format!(
                    "\"{spelled}\" is not a cluster operator's key: purlis reads one as \
                     ed25519: and 64 lower-case hex digits"
                ))
            })
    }

    /// Its spelling, `ed25519:<64 hex>`.
    pub fn spelled(&self) -> String {
        format!("{ED25519}{}", hex_of(&self.0))
    }

    /// A statement whose `text` this key signed, as `signature` (128 lower-case hex), read;
    /// refused where the signature is not this key's over exactly that text, or the text is
    /// not a statement.
    pub fn verified(&self, text: &str, signature: &str) -> Result<Vouched, Refused> {
        self.signed(text, signature)?;
        Vouched::from_text(text)
    }

    /// Whether `signature` is this key's over exactly `text`.
    fn signed(&self, text: &str, signature: &str) -> Result<(), Refused> {
        let signed = signature_bytes(signature).ok_or_else(|| {
            Refused::NotVouched("the cluster operator's signature is not one purlis reads".into())
        })?;
        ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, self.0)
            .verify(text.as_bytes(), &signed)
            .map_err(|_| {
                Refused::NotVouched(
                    "the statement about this runner is not signed by the cluster operator's \
                     key this desktop pinned when the pool was added, so purlis refused it"
                        .into(),
                )
            })
    }
}

/// **A statement as a Claim's status carries it** (`status.linkKey`): its text, the current
/// key's signature, and during a roll the next key's (#3305).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signed {
    pub text: String,
    pub signature: String,
    pub next_signature: Option<String>,
}

/// **What a Pool's status says of its operator** (#2056, #3305), as the desktop read it: each
/// field as written, checked only as the desktop pins or follows it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PoolSeen {
    /// `status.signingKey`.
    pub signing_key: Option<String>,
    /// `status.cluster`: the uid of the cluster's `kube-system` namespace.
    pub cluster: Option<String>,
    /// `status.nextSigningKey`, during a roll.
    pub next_signing_key: Option<String>,
    /// `status.nextSigningKeyVouch`, its statement and signature, during a roll.
    pub next_signing_key_vouch: Option<(String, String)>,
    /// Whether its spec opts the pool in to minting GitHub App tokens in the cluster (#4204):
    /// [`mints_in_cluster`].
    pub mints_in_cluster: bool,
}

/// **Whether a Pool's `spec` opts it in to minting in the cluster** (#4204), read from the admin's
/// Pool, never chosen by the person. `true` alone, with the key and the allow-list that go with
/// it: anything else is not opted in.
pub fn mints_in_cluster(spec: &serde_json::Value) -> bool {
    spec.pointer("/forge/mintInCluster") == Some(&serde_json::Value::Bool(true))
        && spec
            .pointer("/forge/appKeySecretRef/name")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|name| !name.is_empty())
        && spec
            .pointer("/forge/repos")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|repos| !repos.is_empty())
}

impl PoolSeen {
    /// What `status`, a Pool's status as JSON, says.
    pub fn from_status(status: &serde_json::Value) -> PoolSeen {
        let text = |path: &str| {
            status
                .pointer(path)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        PoolSeen {
            signing_key: text("/signingKey"),
            cluster: text("/cluster"),
            next_signing_key: text("/nextSigningKey"),
            next_signing_key_vouch: text("/nextSigningKeyVouch/statement")
                .zip(text("/nextSigningKeyVouch/signature")),
            mints_in_cluster: false,
        }
    }
}

/// The first line of the current key's word for the next one, and its version (#3305).
pub const ROLL_HEADER: &str = "purlis cluster operator key roll v1";

/// The longest a key roll's word holds from now: the operator signs one for seven days, and
/// purlis reads none that claims to hold an hour past that, whatever signed it.
pub const ROLL_MOST_AHEAD: u64 = 7 * 24 * 3600 + 3600;

/// **The current operator key's word for the next one** (#3305): in the cluster `cluster`, the
/// key `from` rolls to the key `to`, until `until` (seconds since the epoch). Signed as its text,
/// one `name: value` line each, in this order, after [`ROLL_HEADER`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyRoll {
    pub cluster: String,
    pub from: OperatorKey,
    pub to: OperatorKey,
    pub until: u64,
}

impl KeyRoll {
    /// Its text, exactly as the operator signs it.
    pub fn text(&self) -> String {
        format!(
            "{ROLL_HEADER}\ncluster: {}\nfrom: {}\nto: {}\nuntil: {}\n",
            self.cluster,
            self.from.spelled(),
            self.to.spelled(),
            self.until
        )
    }

    /// A key roll from its text: exactly [`KeyRoll::text`]'s lines. Anything else is refused.
    pub fn from_text(text: &str) -> Result<KeyRoll, String> {
        let bad =
            || "the cluster operator's word for its next key is not one purlis reads".to_owned();
        let body = text
            .strip_prefix(ROLL_HEADER)
            .and_then(|rest| rest.strip_prefix('\n'))
            .and_then(|rest| rest.strip_suffix('\n'))
            .ok_or_else(bad)?;
        let lines: Vec<&str> = body.split('\n').collect();
        let names = ["cluster", "from", "to", "until"];
        if lines.len() != names.len() {
            return Err(bad());
        }
        let mut values = Vec::with_capacity(names.len());
        for (line, name) in lines.iter().zip(names) {
            let value = line
                .strip_prefix(name)
                .and_then(|rest| rest.strip_prefix(": "))
                .filter(|value| plain(value))
                .ok_or_else(bad)?;
            values.push(value);
        }
        let until = Some(values[3])
            .filter(|digits| digits.len() <= 20 && digits.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|digits| digits.parse::<u64>().ok())
            .ok_or_else(bad)?;
        let roll = KeyRoll {
            cluster: Some(values[0])
                .filter(|cluster| is_uid(cluster))
                .ok_or_else(bad)?
                .to_owned(),
            from: OperatorKey::from_spelled(values[1]).map_err(|_| bad())?,
            to: OperatorKey::from_spelled(values[2]).map_err(|_| bad())?,
            until,
        };
        // Read back byte for byte, so nothing but this text was signed.
        if roll.text() != text {
            return Err(bad());
        }
        Ok(roll)
    }
}

/// **What a desktop pins when it adds a cluster pool** (#2056, #3305): the operator's key and the
/// cluster the pool is in. A statement signed by another key, or made in another cluster, is
/// refused; the key moves only as the pinned key vouches for its next one ([`PoolPin::follow`]).
///
/// Held in memory here: storing a pool's pin, and storing it again as it moves, is #3292's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolPin {
    pub key: OperatorKey,
    /// The uid of the pool's cluster's `kube-system` namespace.
    pub cluster: String,
}

/// **What a desktop does with a Pool's status it read** ([`PoolPin::followed`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Following {
    /// Its pin holds as it is: no roll, or one it has followed already.
    Stays,
    /// The pinned key vouches for its next one: the pin moves there, and no one is asked.
    Moves(PoolPin),
    /// The Pool names a next key the pinned key does not vouch for: the pin holds, and why.
    NotFollowed(String),
}

impl PoolPin {
    /// **The pin of a pool being added**, from its status: its key and cluster, each as purlis
    /// reads one.
    pub fn first(pool: &str, seen: &PoolSeen) -> Result<PoolPin, Refused> {
        let key = seen.signing_key.as_deref().ok_or_else(|| {
            Refused::NotVouched(format!(
                "the pool {pool} names no cluster operator's key yet: is purlis's cluster \
                 operator running there?"
            ))
        })?;
        let cluster = seen
            .cluster
            .as_deref()
            .filter(|cluster| is_uid(cluster))
            .ok_or_else(|| {
                Refused::NotVouched(format!(
                    "the pool {pool} names no cluster purlis reads: update purlis's cluster \
                     operator there so it and this purlis are of one release"
                ))
            })?;
        Ok(PoolPin {
            key: OperatorKey::from_spelled(key)?,
            cluster: cluster.to_owned(),
        })
    }

    /// **A statement signed with this pin's key, made in its cluster**, read: its `signature`
    /// or, during a roll, its `next_signature`, must be the pinned key's over exactly its text.
    pub fn verified(&self, signed: &Signed) -> Result<Vouched, Refused> {
        if let Err(refused) = self.key.signed(&signed.text, &signed.signature) {
            match signed.next_signature.as_deref() {
                Some(next) if self.key.signed(&signed.text, next).is_ok() => {}
                _ => return Err(refused),
            }
        }
        let vouched = Vouched::from_text(&signed.text)?;
        if vouched.cluster != self.cluster {
            return Err(Refused::NotVouched(
                "the cluster operator's statement was made in another cluster than the one this \
                 desktop pinned when the pool was added, so purlis refused it"
                    .into(),
            ));
        }
        Ok(vouched)
    }

    /// **Whether this pin follows a roll the Pool's status, `seen` at `now`, names** (#3305).
    ///
    /// - The Pool in another cluster, or naming none: refused.
    /// - The Pool's key is the pinned one, and it names a next key the pinned key vouches for in
    ///   this cluster, from this key to that one, holding now and for no more than seven days
    ///   and an hour: the pin moves.
    /// - The Pool's next key is the pinned one already: the pin stays.
    /// - The Pool's key is neither: refused, and the pool is added again, as for any key this
    ///   desktop did not pin. A desktop that missed a roll asks again this way.
    pub fn followed(&self, seen: &PoolSeen, now: u64) -> Result<Following, Refused> {
        if seen.cluster.as_deref() != Some(self.cluster.as_str()) {
            return Err(Refused::NotVouched(
                "the pool's cluster operator names another cluster than the one this desktop \
                 pinned when the pool was added, so purlis refused it"
                    .into(),
            ));
        }
        let pinned = self.key.spelled();
        if seen.next_signing_key.as_deref() == Some(pinned.as_str()) {
            return Ok(Following::Stays);
        }
        if seen.signing_key.as_deref() != Some(pinned.as_str()) {
            return Err(Refused::NotVouched(
                "the pool's cluster operator signs with another key than the one this desktop \
                 pinned, and that key did not vouch for it while this desktop looked: add the \
                 pool again if the team rolled its key"
                    .into(),
            ));
        }
        let Some(next) = seen.next_signing_key.as_deref() else {
            return Ok(Following::Stays);
        };
        let not = |why: &str| Ok(Following::NotFollowed(why.to_owned()));
        let Some((text, signature)) = seen.next_signing_key_vouch.as_ref() else {
            return not("the pool names a next key with no word for it from the pinned key");
        };
        if self.key.signed(text, signature).is_err() {
            return not("the word for the pool's next key is not signed by the pinned key");
        }
        let roll = match KeyRoll::from_text(text) {
            Ok(roll) => roll,
            Err(why) => return Ok(Following::NotFollowed(why)),
        };
        if roll.cluster != self.cluster || roll.from != self.key || roll.to.spelled() != next {
            return not("the pinned key's word is for another cluster, key or next key");
        }
        if roll.until <= now {
            return not("the pinned key's word for the next key no longer holds");
        }
        if roll.until > now.saturating_add(ROLL_MOST_AHEAD) {
            return not(
                "the pinned key's word for the next key says it holds for more than seven days; check this machine's clock",
            );
        }
        Ok(Following::Moves(PoolPin {
            key: roll.to,
            cluster: self.cluster.clone(),
        }))
    }

    /// **Moves this pin as [`PoolPin::followed`] says**, in memory: whether it moved. A refusal
    /// leaves it as it was. Storing the moved pin is #3292's, at this seam: whoever holds the
    /// pool's pin writes it back where it keeps it when this answers `true`.
    pub fn follow(&mut self, seen: &PoolSeen, now: u64) -> Result<bool, Refused> {
        match self.followed(seen, now)? {
            Following::Moves(moved) => {
                *self = moved;
                Ok(true)
            }
            Following::Stays | Following::NotFollowed(_) => Ok(false),
        }
    }
}

/// Shows the spelling of a public key, which is no secret.
impl std::fmt::Debug for OperatorKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OperatorKey({})", self.spelled())
    }
}

fn signature_bytes(hex: &str) -> Option<Vec<u8>> {
    if !is_lower_hex(hex, 128) {
        return None;
    }
    (0..64)
        .map(|at| u8::from_str_radix(&hex[at * 2..at * 2 + 2], 16).ok())
        .collect()
}

/// **The path the runner's pod hands its link key over at** (`docs/cluster-operator.md`): a
/// folder in memory the operator's pod template mounts, where the pod's first step writes the
/// key's private half and the runner's pairing takes it, leaving nothing there.
pub const KEY_MADE_AT_START: &str = "/run/purlis-link/link-key";

/// The container a runner's pod runs `purlis serve` in.
pub const RUNNER_CONTAINER: &str = "runner";

/// Where `purlis` is in a runner image (`docs/runner-image.md`).
const PURLIS: &str = crate::dockerpool::PURLIS;

/// **`kubectl`, held as data**: the program, and the kubeconfig and context it is pointed at,
/// each passed on every run, so nothing it does reads another cluster than the pool's.
///
/// **The kubectl found on this machine runs in an environment of its own** (#3292,
/// [`Kubectl::on_this_machine`]): `KUBECONFIG` is never handed on ([`CLEARED`]), so the
/// kubeconfig is only ever the one named with `--kubeconfig`, or kubectl's own default, and never
/// one a chat's environment pointed at; and its `PATH` is the folder it is in, the one its link
/// leads to and the machine's own folders ([`crate::programs::SYSTEM_BIN`]), where a
/// kubeconfig's credential plugin is found when it is installed beside kubectl or system-wide.
/// One elsewhere is named in the kubeconfig by its absolute path.
#[derive(Debug, Clone)]
pub struct Kubectl {
    program: PathBuf,
    kubeconfig: Option<PathBuf>,
    context: Option<String>,
    /// The `PATH` its runs are given, and that they are handed no `KUBECONFIG`: set for the
    /// kubectl found on this machine ([`Kubectl::on_this_machine`]), the one the window runs.
    path: Option<std::ffi::OsString>,
    /// The folder its discovery and HTTP caches are kept in (`--cache-dir`), where one is named.
    cache_dir: Option<PathBuf>,
    /// How long each run is waited for, where it is held to a time ([`Kubectl::within`]).
    within: Option<std::time::Duration>,
    /// When every run of a check is up, where they share a time ([`Kubectl::all_within`]).
    until: Option<std::time::Instant>,
}

/// The variables a run of kubectl is never handed from this process: the kubeconfig is named,
/// never inherited (#3292).
pub const CLEARED: [&str; 1] = ["KUBECONFIG"];

/// Where a `kubectl` command is looked for first, before the home's installers and `PATH`:
/// Homebrew's, `/usr/local`'s, Docker Desktop's own, the system's and snap's.
const KUBECTL_INSTALLED: &[&str] = &[
    "/opt/homebrew/bin/kubectl",
    "/usr/local/bin/kubectl",
    "/Applications/Docker.app/Contents/Resources/bin/kubectl",
    "/usr/bin/kubectl",
    "/snap/bin/kubectl",
];

/// Where Rancher Desktop, OrbStack and Docker Desktop link `kubectl` in the home folder, each
/// taken only from its installer's own folder ([`crate::dockerpool::its_own_in`]).
const KUBECTL_IN_HOME: &[&str] = &[
    ".rd/bin/kubectl",
    ".orbstack/bin/kubectl",
    ".docker/bin/kubectl",
];

/// The most purlis reads of what kubectl prints for one object.
const MOST_PRINTED: u64 = 1024 * 1024;

/// The most purlis keeps of what kubectl says on standard error, to show it.
const MOST_SAID: u64 = 16 * 1024;

impl Kubectl {
    pub fn at(program: impl Into<PathBuf>) -> Kubectl {
        Kubectl {
            program: program.into(),
            kubeconfig: None,
            context: None,
            path: None,
            cache_dir: None,
            within: None,
            until: None,
        }
    }

    /// This kubectl, each run ended and refused once `within` is up (#4071): what a check that
    /// must fail closed runs, so a cluster that does not answer never holds it.
    #[must_use]
    pub fn within(mut self, within: std::time::Duration) -> Kubectl {
        self.within = Some(within);
        self
    }

    /// This kubectl, every run from now on ended and refused once `all` is up, whatever each is
    /// given: what a check of several runs shares, so it answers within one time.
    #[must_use]
    pub fn all_within(mut self, all: std::time::Duration) -> Kubectl {
        self.until = Some(std::time::Instant::now() + all);
        self
    }

    /// How long a run started now is given.
    fn time_left(&self) -> Option<std::time::Duration> {
        let left = self
            .until
            .map(|until| until.saturating_duration_since(std::time::Instant::now()));
        match (self.within, left) {
            (Some(within), Some(left)) => Some(within.min(left)),
            (within, left) => within.or(left),
        }
    }

    /// This kubectl, keeping its caches in `folder` (`--cache-dir`) rather than the home's.
    #[must_use]
    pub fn with_cache_dir(mut self, folder: impl Into<PathBuf>) -> Kubectl {
        self.cache_dir = Some(folder.into());
        self
    }

    /// **The program this kubectl is**, by its real path, links followed (#3292).
    pub fn real_program(&self) -> Result<PathBuf, Refused> {
        self.program.canonicalize().map_err(|why| {
            Refused::Other(format!(
                "{} could not be read as a program: {why}",
                self.program.display()
            ))
        })
    }

    /// **Where a kubeconfig's credential plugin `command` is**, as a run of this kubectl finds
    /// it (#3292): an absolute command as it is, a bare name in the folders its runs are given on
    /// `PATH`, links not followed. `None` where it is not found.
    pub fn plugin_at(&self, command: &str) -> Option<PathBuf> {
        let named = std::path::Path::new(command);
        let found = if named.is_absolute() {
            Some(named.to_path_buf())
        } else if command.contains('/') {
            None
        } else {
            self.path.as_ref().and_then(|path| {
                std::env::split_paths(path)
                    .map(|folder| folder.join(command))
                    .find(|candidate| crate::programs::runnable(candidate))
            })
        };
        found.filter(|found| crate::programs::runnable(found))
    }

    /// **The `kubectl` command on this machine, by its absolute path** (#3292), found as
    /// Docker's is ([`crate::dockerpool::Docker::on_this_machine`]): the first of the places
    /// installers put it (`KUBECTL_INSTALLED`, then under `home`), else the first on `path`, of
    /// its absolute folders alone. A window started from the Finder has a `PATH` that names
    /// none. Its runs are given a `PATH` of its own folders and the machine's (above). Refused,
    /// saying how to get one, where there is none.
    pub fn on_this_machine(
        path: Option<&str>,
        home: Option<&std::path::Path>,
    ) -> Result<Kubectl, Refused> {
        Kubectl::found_in(KUBECTL_INSTALLED, path, home)
    }

    fn found_in(
        installed: &[&str],
        path: Option<&str>,
        home: Option<&std::path::Path>,
    ) -> Result<Kubectl, Refused> {
        let installed = installed.iter().map(PathBuf::from);
        let in_home = home.into_iter().flat_map(|home| {
            KUBECTL_IN_HOME
                .iter()
                .filter(move |at| crate::dockerpool::its_own_in(home, at))
                .map(move |at| home.join(at))
        });
        let on_path = path
            .into_iter()
            .flat_map(|path| path.split(':'))
            .filter(|dir| dir.starts_with('/'))
            .map(|dir| std::path::Path::new(dir).join("kubectl"));
        let program = installed
            .chain(in_home)
            .chain(on_path)
            .find(|candidate| crate::programs::runnable(candidate))
            .ok_or_else(|| {
                Refused::Other(
                    "purlis found no kubectl on this machine; install kubectl, with Homebrew or \
                     your cluster's own instructions, and try again"
                        .into(),
                )
            })?;
        // Its own folder, the one its link leads to, then the machine's own: never a folder of
        // the home or of the window's `PATH`, so nothing put there is run as a credential
        // plugin by a run of this machine's own.
        let linked = program
            .canonicalize()
            .ok()
            .and_then(|real| real.parent().map(PathBuf::from));
        let mut folders: Vec<PathBuf> = Vec::new();
        for folder in program
            .parent()
            .map(PathBuf::from)
            .into_iter()
            .chain(linked)
            .chain(crate::programs::SYSTEM_BIN.iter().map(PathBuf::from))
        {
            if folder.is_absolute() && !folders.contains(&folder) {
                folders.push(folder);
            }
        }
        let path = std::env::join_paths(folders).ok();
        Ok(Kubectl {
            path,
            ..Kubectl::at(program)
        })
    }

    /// This kubectl, pointed at `context` of the kubeconfig at `kubeconfig`: the two a cluster
    /// pool names (#3292).
    #[must_use]
    pub fn pointed_at(self, kubeconfig: &std::path::Path, context: &str) -> Kubectl {
        self.with_kubeconfig(kubeconfig).with_context(context)
    }

    /// **What each run is given** besides its words: the variables it is handed, and those it
    /// is not ([`CLEARED`]). A connector through this kubectl gives each of its runs the same.
    pub fn environment(
        &self,
    ) -> (
        Vec<(std::ffi::OsString, std::ffi::OsString)>,
        Vec<std::ffi::OsString>,
    ) {
        let Some(path) = &self.path else {
            // A kubectl held by its path alone (a test's, against its own cluster) runs as it is.
            return (Vec::new(), Vec::new());
        };
        (
            // Its own preferences file is off: `off` is kubectl's word for that.
            vec![
                ("PATH".into(), path.clone()),
                ("KUBERC".into(), "off".into()),
            ],
            CLEARED.iter().map(std::ffi::OsString::from).collect(),
        )
    }

    /// This kubectl, reading the kubeconfig at `path`.
    #[must_use]
    pub fn with_kubeconfig(mut self, path: impl Into<PathBuf>) -> Kubectl {
        self.kubeconfig = Some(path.into());
        self
    }

    /// This kubectl, on the context `context`.
    #[must_use]
    pub fn with_context(mut self, context: &str) -> Kubectl {
        self.context = Some(context.to_owned());
        self
    }

    pub fn program(&self) -> &std::path::Path {
        &self.program
    }

    /// The words before every command: the kubeconfig and the context.
    fn pointed(&self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(path) = &self.kubeconfig {
            args.push("--kubeconfig".to_owned());
            args.push(path.display().to_string());
        }
        if let Some(context) = &self.context {
            args.push("--context".to_owned());
            args.push(context.clone());
        }
        if let Some(folder) = &self.cache_dir {
            args.push("--cache-dir".to_owned());
            args.push(folder.display().to_string());
        }
        args
    }

    /// `args`, after the words that point kubectl at the pool's cluster.
    pub fn args(&self, args: &[&str]) -> Vec<String> {
        let mut all = self.pointed();
        all.extend(args.iter().map(|arg| (*arg).to_owned()));
        all
    }

    /// The connector's words to `pod`'s runner in `namespace`: its bridge, through
    /// `kubectl exec -i` (ADR 0078 §2).
    pub fn connect_args(&self, namespace: &str, pod: &str) -> Vec<String> {
        self.args(&[
            "exec",
            "-i",
            "--namespace",
            namespace,
            "--container",
            RUNNER_CONTAINER,
            &format!("pod/{pod}"),
            "--",
            PURLIS,
            "bridge",
        ])
    }

    /// **The words that run `pod`'s runner's own `purlis profile sign-in` of its profile `name`**
    /// in `namespace` (#3819), to `provider`'s plan where one is named, by `typed`: the same
    /// sign-in a Docker runner runs ([`crate::dockerpool::sign_in_words`]), as the pod's own
    /// user, in the runner's container.
    pub fn sign_in_args(
        &self,
        namespace: &str,
        pod: &str,
        typed: crate::dockerpool::Typed,
        name: &str,
        provider: Option<&str>,
    ) -> Vec<String> {
        let mut args = self.args(&[
            "exec",
            typed.exec_flag(),
            "--namespace",
            namespace,
            "--container",
            RUNNER_CONTAINER,
            &format!("pod/{pod}"),
            "--",
        ]);
        args.extend(crate::dockerpool::sign_in_words(typed, name, provider));
        args
    }

    /// **The words that run `pod`'s runner's own `purlis` with `words`** in `namespace`, with
    /// pipes (#3819): its version, a profile's sign-out.
    pub fn purlis_args(&self, namespace: &str, pod: &str, words: &[&str]) -> Vec<String> {
        let mut args = self.args(&[
            "exec",
            "-i",
            "--namespace",
            namespace,
            "--container",
            RUNNER_CONTAINER,
            &format!("pod/{pod}"),
            "--",
            PURLIS,
        ]);
        args.extend(words.iter().map(|word| (*word).to_owned()));
        args
    }

    /// The pairing's words: the bridge's pairing, taking the key the pod made as it started.
    pub fn pair_args(&self, namespace: &str, pod: &str) -> Vec<String> {
        let mut args = self.connect_args(namespace, pod);
        args.extend([
            "--pair".to_owned(),
            "--key-made-at-start".to_owned(),
            KEY_MADE_AT_START.to_owned(),
        ]);
        args
    }

    /// The words that tell a paired runner where the mint port is (#4207), on standard input.
    pub fn tell_mint_port_args(&self, namespace: &str, pod: &str) -> Vec<String> {
        let mut args = self.connect_args(namespace, pod);
        args.push("--mint-port".to_owned());
        args
    }

    /// The words that tell a paired runner again at a link (#4330). A runner from before them
    /// refuses the word, so it keeps what it followed.
    pub fn tell_mint_port_again_args(&self, namespace: &str, pod: &str) -> Vec<String> {
        let mut args = self.tell_mint_port_args(namespace, pod);
        args.push("--again".to_owned());
        args
    }

    /// Runs kubectl with `args`, handing it `input`, and answers what it printed, up to `most`
    /// bytes. What it says on standard error is only shown, never read to decide anything.
    pub fn run(&self, args: &[String], input: Option<&str>, most: u64) -> Result<String, Refused> {
        let what = || {
            args.iter()
                .find(|arg| !arg.starts_with('-') && !self.pointed().contains(arg))
                .cloned()
                .unwrap_or_default()
        };
        let mut command = std::process::Command::new(&self.program);
        let (env, cleared) = self.environment();
        command.envs(env);
        for name in cleared {
            command.env_remove(name);
        }
        command.args(args);
        let ran = crate::boundedrun::run_within(
            &mut command,
            input,
            most,
            MOST_SAID,
            self.time_left(),
        )
        .map_err(|why| {
            use crate::boundedrun::Unanswered;
            Refused::Other(match why {
                Unanswered::NotStarted(why) => {
                    format!("{} could not be run: {why}", self.program.display())
                }
                Unanswered::PrintedTooMuch => format!(
                    "kubectl {} printed more than the {most} bytes purlis reads of it, so it was \
                     ended. {}",
                    what(),
                    purlis_session_protocol::fixedlimits::RUNNER_OUTPUT.said()
                ),
                Unanswered::NotFinished(why) => {
                    format!("{} did not finish: {why}", self.program.display())
                }
            })
        })?;
        if !ran.status.success() {
            return Err(Refused::Other(format!(
                "kubectl {} failed: {}",
                what(),
                String::from_utf8_lossy(&ran.said).trim()
            )));
        }
        String::from_utf8(ran.printed)
            .map_err(|_| Refused::Other("kubectl answered with something that is not text".into()))
    }

    fn json(&self, args: &[&str], input: Option<&str>) -> Result<serde_json::Value, Refused> {
        let out = self.run(&self.args(args), input, MOST_PRINTED)?;
        serde_json::from_str(&out).map_err(|_| {
            Refused::Other(format!(
                "kubectl {} did not answer JSON",
                args.first().copied().unwrap_or_default()
            ))
        })
    }

    /// **Who this kubeconfig signs in as**, as the cluster says (`kubectl auth whoami`): the
    /// requester a claim this desktop makes is stamped with.
    pub fn whoami(&self) -> Result<String, Refused> {
        let answer = self.json(&["auth", "whoami", "--output", "json"], None)?;
        answer
            .pointer("/status/userInfo/username")
            .and_then(serde_json::Value::as_str)
            .filter(|name| plain(name))
            .map(str::to_owned)
            .ok_or_else(|| Refused::Other("the cluster did not say who this kubeconfig is".into()))
    }

    /// **The contexts of the kubeconfig this kubectl reads** (#3292), as kubectl itself lists
    /// them (`kubectl config view`, which leaves every credential out), and the one it is on.
    pub fn contexts(&self) -> Result<(Vec<String>, Option<String>), Refused> {
        let view = self.json(&["config", "view", "--output", "json"], None)?;
        let names = view
            .get("contexts")
            .and_then(serde_json::Value::as_array)
            .map(|contexts| {
                contexts
                    .iter()
                    .filter_map(|one| one.get("name").and_then(serde_json::Value::as_str))
                    .filter(|name| plain(name))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let current = view
            .get("current-context")
            .and_then(serde_json::Value::as_str)
            .filter(|name| plain(name))
            .map(str::to_owned);
        Ok((names, current))
    }

    /// **This kubectl's context, whole, as one kubeconfig** (#3292): `kubectl config view
    /// --minify --raw --flatten`, the context it is pointed at with its cluster and user and
    /// what each names inlined, for a cluster pool to keep as its own copy. Never shown.
    pub fn context_kept(&self) -> Result<String, Refused> {
        let kept = self.run(
            &self.args(&[
                "config",
                "view",
                "--minify",
                "--raw",
                "--flatten",
                "--output",
                "json",
            ]),
            None,
            MOST_PRINTED,
        )?;
        if kept.trim().is_empty() {
            return Err(Refused::Other(
                "kubectl gave nothing of the kubeconfig's context to keep".into(),
            ));
        }
        Ok(kept)
    }

    /// **The namespace this kubectl's context names**, where a cluster pool's claims are made
    /// (#3292): kubectl's own `default` where it names none.
    pub fn namespace(&self) -> Result<String, Refused> {
        let view = self.json(&["config", "view", "--minify", "--output", "json"], None)?;
        let named = view
            .pointer("/contexts/0/context/namespace")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("default");
        if !is_label(named) {
            return Err(Refused::Other(format!(
                "the context names the namespace \"{named}\", which is not a namespace's name"
            )));
        }
        Ok(named.to_owned())
    }

    /// **What the Claim `namespace`/`name`'s `Ready` condition says**, to show why its runner is
    /// not vouched for yet; `None` where it says nothing. Only shown, never read to decide.
    pub fn claim_said(&self, namespace: &str, name: &str) -> Option<String> {
        let held = self
            .json(
                &[
                    "get",
                    "claims.runners.purlis.dev",
                    name,
                    "--namespace",
                    namespace,
                    "--output",
                    "json",
                ],
                None,
            )
            .ok()?;
        let ready = held
            .pointer("/status/conditions")?
            .as_array()?
            .iter()
            .find(|one| one.get("type").and_then(serde_json::Value::as_str) == Some("Ready"))?;
        let field = |name: &str| ready.get(name).and_then(serde_json::Value::as_str);
        let said: Vec<&str> = [field("reason"), field("message")]
            .into_iter()
            .flatten()
            .filter(|said| !said.is_empty())
            .collect();
        (!said.is_empty()).then(|| crate::shown::short(&said.join(": ")))
    }

    /// **Where the Claim `namespace`/`name`'s runner has got to** (#3709), for a window waiting on
    /// it; `None` where it is not on its way, or the Claim could not be read. Only shown, never
    /// read to decide.
    pub fn claim_stage(&self, namespace: &str, name: &str) -> Option<RunnerStage> {
        let held = self
            .json(
                &[
                    "get",
                    "claims.runners.purlis.dev",
                    name,
                    "--namespace",
                    namespace,
                    "--output",
                    "json",
                ],
                None,
            )
            .ok()?;
        stage_of(&held)
    }

    /// **What a pool's status says of its operator**: its key, its cluster and any roll.
    pub fn pool(&self, pool: &str) -> Result<PoolSeen, Refused> {
        let held = self.json(
            &["get", "pools.runners.purlis.dev", pool, "--output", "json"],
            None,
        )?;
        Ok(PoolSeen {
            mints_in_cluster: held.get("spec").is_some_and(mints_in_cluster),
            ..PoolSeen::from_status(held.get("status").unwrap_or(&serde_json::Value::Null))
        })
    }

    /// **The pin of a pool being added** ([`PoolPin::first`]): the operator's key and the
    /// cluster its status names.
    pub fn pool_pin(&self, pool: &str) -> Result<PoolPin, Refused> {
        PoolPin::first(pool, &self.pool(pool)?)
    }

    /// **Claims a runner of `pool` in `namespace`**, as `requester` (who this kubeconfig is,
    /// [`Kubectl::whoami`]), kept or fresh as `keeping` says ([`kept_for`]): the Claim's uid,
    /// which the operator's statement must name.
    pub fn claim(
        &self,
        namespace: &str,
        name: &str,
        pool: &str,
        requester: &str,
        keeping: Keeping,
    ) -> Result<String, Refused> {
        self.claim_marked(namespace, name, pool, requester, keeping, None)
    }

    /// [`Kubectl::claim`], the Claim marked as this desktop's (`desktop`, [`DESKTOP_ANNOTATION`])
    /// where one is given (#3292).
    pub fn claim_marked(
        &self,
        namespace: &str,
        name: &str,
        pool: &str,
        requester: &str,
        keeping: Keeping,
        desktop: Option<&str>,
    ) -> Result<String, Refused> {
        let mut claim = claim_definition(namespace, name, pool, requester, keeping);
        if let Some(desktop) = desktop {
            claim["metadata"]["annotations"] = serde_json::json!({ DESKTOP_ANNOTATION: desktop });
        }
        let made = self.json(
            &["create", "--filename", "-", "--output", "json"],
            Some(&claim.to_string()),
        )?;
        made.pointer("/metadata/uid")
            .and_then(serde_json::Value::as_str)
            .filter(|uid| plain(uid))
            .map(str::to_owned)
            .ok_or_else(|| Refused::Other("the cluster made the claim but gave no uid".into()))
    }

    /// The statement and its signatures on the Claim `namespace`/`name`'s status, where the
    /// operator has written one. **A Claim being deleted vouches for nothing** (#4071): its
    /// finalizer keeps it there, statement and all, while its runner is taken away.
    pub fn statement(&self, namespace: &str, name: &str) -> Result<Option<Signed>, Refused> {
        let held = self.json(
            &[
                "get",
                "claims.runners.purlis.dev",
                name,
                "--namespace",
                namespace,
                "--output",
                "json",
            ],
            None,
        )?;
        if held
            .pointer("/metadata/deletionTimestamp")
            .is_some_and(|since| !since.is_null())
        {
            return Ok(None);
        }
        let field = |name: &str| {
            held.pointer(&format!("/status/linkKey/{name}"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        Ok(field("statement")
            .zip(field("signature"))
            .map(|(text, signature)| Signed {
                text,
                signature,
                next_signature: field("nextSignature"),
            }))
    }

    /// **Whether the runner of the Claim `namespace`/`name` of the Pool `pool` is still serving**,
    /// before a value goes to it: neither is stopped and the Claim's `KeyPublished` condition says
    /// `Vouched`. None of this is signed, so it only ever refuses: the signed statement is what
    /// vouches ([`vouched_for`]).
    pub fn serving(&self, namespace: &str, name: &str, pool: &str) -> Result<(), Refused> {
        let claim = self.json(
            &[
                "get",
                "claims.runners.purlis.dev",
                name,
                "--namespace",
                namespace,
                "--output",
                "json",
            ],
            None,
        )?;
        let of_the_pool = self.json(
            &["get", "pools.runners.purlis.dev", pool, "--output", "json"],
            None,
        )?;
        let stopped = |held: &serde_json::Value| {
            held.pointer("/spec/stopped") == Some(&serde_json::Value::Bool(true))
        };
        let vouched = claim
            .pointer("/status/conditions")
            .and_then(serde_json::Value::as_array)
            .and_then(|conditions| {
                conditions.iter().find(|one| {
                    one.get("type").and_then(serde_json::Value::as_str) == Some("KeyPublished")
                })
            })
            .is_some_and(|published| {
                let field = |name: &str| published.get(name).and_then(serde_json::Value::as_str);
                field("status") == Some("True") && field("reason") == Some("Vouched")
            });
        if stopped(&claim) || stopped(&of_the_pool) || !vouched {
            return Err(Refused::NotVouched(format!(
                "the runner of claim {namespace}/{name} is stopped, or the cluster operator's word \
                 on it is no longer that it vouches for it"
            )));
        }
        Ok(())
    }

    /// **The uid of the pod `namespace`/`pod`** as the cluster says now: a pod made again under
    /// its name has another. Refused where it is being deleted or names none.
    pub fn pod_uid(&self, namespace: &str, pod: &str) -> Result<String, Refused> {
        let held = self.json(
            &[
                "get",
                "pods",
                pod,
                "--namespace",
                namespace,
                "--output",
                "json",
            ],
            None,
        )?;
        let deleting = held
            .pointer("/metadata/deletionTimestamp")
            .is_some_and(|since| !since.is_null());
        held.pointer("/metadata/uid")
            .and_then(serde_json::Value::as_str)
            .filter(|uid| !deleting && plain(uid))
            .map(str::to_owned)
            .ok_or_else(|| {
                Refused::NotVouched(format!("the pod {namespace}/{pod} is not there as it was"))
            })
    }

    /// **The Claim `namespace`/`name` as the cluster holds it** (#3292): its uid, who asked for
    /// it, the desktop it is marked as ([`DESKTOP_ANNOTATION`]) and whether it is kept (#3819);
    /// `None` where there is none.
    pub fn claim_seen(&self, namespace: &str, name: &str) -> Result<Option<ClaimSeen>, Refused> {
        let Some(held) = self.claim_json(namespace, name)? else {
            return Ok(None);
        };
        let text = |path: &str| {
            held.pointer(path)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        Ok(Some(ClaimSeen {
            uid: text("/metadata/uid").unwrap_or_default(),
            requester: text("/spec/requester"),
            desktop: held
                .pointer("/metadata/annotations")
                .and_then(|marks| marks.get(DESKTOP_ANNOTATION))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            kept: held.pointer("/spec/kept") == Some(&serde_json::Value::Bool(true)),
        }))
    }

    /// **`status.mint` of the Claim `namespace`/`name`** (#4207), `(address, operatorKey)`, where
    /// the operator wrote it.
    pub fn claim_mint(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<Option<(String, String)>, Refused> {
        let held = self.claim_json(namespace, name)?;
        let text = |path: &str| {
            held.as_ref()?
                .pointer(path)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        Ok(text("/status/mint/address").zip(text("/status/mint/operatorKey")))
    }

    /// **Adds `repos` to `Claim.spec.forge.repos`** (#4207), as the Claim's requester, whose word the
    /// cluster's policy holds it to; what the Claim names already stays, for the runner's other
    /// chats. Written only where it adds something, and only over the Claim as it was read, so two
    /// chats starting at once both arrive. Refused past [`MOST_CLAIM_REPOS`].
    pub fn add_claim_repos(
        &self,
        namespace: &str,
        name: &str,
        repos: &[String],
    ) -> Result<(), Refused> {
        let mut tries = CLAIM_WRITES;
        loop {
            tries -= 1;
            match self.add_claim_repos_once(namespace, name, repos) {
                Err(_) if tries > 0 => {}
                done => return done,
            }
        }
    }

    fn add_claim_repos_once(
        &self,
        namespace: &str,
        name: &str,
        repos: &[String],
    ) -> Result<(), Refused> {
        let Some(held) = self.claim_json(namespace, name)? else {
            return Ok(());
        };
        let named: Vec<String> = held
            .pointer("/spec/forge/repos")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|repo| repo.as_str().map(str::to_owned))
            .collect();
        let Some(all) = repos_with(&named, repos) else {
            return Ok(());
        };
        if all.len() > MOST_CLAIM_REPOS {
            return Err(Refused::Other(format!(
                "a claim names at most {MOST_CLAIM_REPOS} repos, so this workspace's were not added"
            )));
        }
        let version = held
            .pointer("/metadata/resourceVersion")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| Refused::Other("the claim holds no resource version".into()))?;
        let patch = serde_json::json!({
            "metadata": { "resourceVersion": version },
            "spec": { "forge": { "repos": all } },
        })
        .to_string();
        self.run(
            &self.args(&[
                "patch",
                "claims.runners.purlis.dev",
                name,
                "--namespace",
                namespace,
                "--type=merge",
                "--patch",
                &patch,
            ]),
            None,
            MOST_PRINTED,
        )
        .map(|_| ())
    }

    /// **Asks the operator to run the Claim `namespace`/`name`'s idle runner again** (#3980), where
    /// its `Idle` condition says it is suspended: the condition's own time is the word, so this
    /// says nothing of any later suspension. Nothing is written otherwise.
    ///
    /// Answers whether this word brings its runner back (#4403, [`suspended_for_being_idle`]).
    pub fn run_again_if_idle(&self, namespace: &str, name: &str) -> Result<bool, Refused> {
        let Some(held) = self.claim_json(namespace, name)? else {
            return Ok(false);
        };
        let Some(since) = idle_since(&held) else {
            return Ok(false);
        };
        let comes_back = suspended_for_being_idle(&held);
        if run_again_word(&held) == Some(since) {
            return Ok(comes_back);
        }
        let patch = serde_json::json!({
            "metadata": { "annotations": { RUN_AGAIN_ANNOTATION: since } },
        })
        .to_string();
        self.run(
            &self.args(&[
                "patch",
                "claims.runners.purlis.dev",
                name,
                "--namespace",
                namespace,
                "--type=merge",
                "--patch",
                &patch,
            ]),
            None,
            MOST_PRINTED,
        )
        .map(|_| comes_back)
    }

    /// **What the Claim `namespace`/`name` says of its deletion** (#3981), where its operator gave
    /// notice: when, in its words. Data, never read to decide.
    pub fn deletion_notice(&self, namespace: &str, name: &str) -> Result<Option<String>, Refused> {
        Ok(self
            .claim_json(namespace, name)?
            .as_ref()
            .and_then(deletion_notice_of))
    }

    /// The Claim `namespace`/`name` as the cluster has it, where it does.
    fn claim_json(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<Option<serde_json::Value>, Refused> {
        let printed = self.run(
            &self.args(&[
                "get",
                "claims.runners.purlis.dev",
                name,
                "--namespace",
                namespace,
                "--ignore-not-found",
                "--output",
                "json",
            ]),
            None,
            MOST_PRINTED,
        )?;
        if printed.trim().is_empty() {
            return Ok(None);
        }
        serde_json::from_str(&printed)
            .map(Some)
            .map_err(|_| Refused::Other("kubectl get did not answer JSON".into()))
    }

    /// Deletes the Claim `namespace`/`name`, and with it its runner; nothing where it is gone
    /// already.
    pub fn delete_claim(&self, namespace: &str, name: &str) -> Result<(), Refused> {
        self.run(
            &self.args(&[
                "delete",
                "claims.runners.purlis.dev",
                name,
                "--namespace",
                namespace,
                "--ignore-not-found",
                "--wait=false",
            ]),
            None,
            MOST_PRINTED,
        )
        .map(|_| ())
    }
}

/// **The annotation that marks a Claim as one desktop's** (#3292): its value is that desktop's
/// label ([`desktop_label`]), so a desktop removes only a Claim it made.
pub const DESKTOP_ANNOTATION: &str = "runners.purlis.dev/desktop";

/// **A desktop's label on the Claims it makes** (#3292): the first 16 hex digits of the SHA-256
/// of its device id, in lower case, the same however the device id is spelled.
pub fn desktop_label(device: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(device.to_ascii_lowercase().as_bytes());
    hex_of(digest.as_slice())[..16].to_owned()
}

/// A Claim as the cluster holds it ([`Kubectl::claim_seen`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimSeen {
    pub uid: String,
    pub requester: Option<String>,
    pub desktop: Option<String>,
    /// Whether it is a kept claim (`spec.kept`, #2057): its runner's home outlives its pod.
    pub kept: bool,
}

/// **Whether a kubeconfig's context can be named to kubectl** (#3292): plain text, at most 253
/// bytes, that cannot read as a flag. A context is named as its kubeconfig names it, which a
/// managed cluster's often does with capitals, colons or underscores, so it is not held to a
/// Kubernetes object's name.
pub fn is_context(context: &str) -> bool {
    plain(context) && context.len() <= 253 && !context.starts_with('-')
}

/// **The credential plugins a kubeconfig names** (#3292): each user's `exec.command`, in `kept`,
/// a kubeconfig as JSON ([`Kubectl::context_kept`]).
pub fn plugins_named(kept: &str) -> Result<Vec<String>, Refused> {
    let kept: serde_json::Value = serde_json::from_str(kept)
        .map_err(|_| Refused::Other("the kubeconfig's context is not one purlis reads".into()))?;
    Ok(kept
        .get("users")
        .and_then(serde_json::Value::as_array)
        .map(|users| {
            users
                .iter()
                .filter_map(|user| user.pointer("/user/exec/command"))
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default())
}

/// **What a claim of a pool is made as** ([`claim_and_pair`]): the cluster's Pool, the namespace
/// the Claim is made in, its name, and the desktop it is marked as ([`DESKTOP_ANNOTATION`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimTo<'a> {
    pub pool: &'a str,
    pub namespace: &'a str,
    pub name: &'a str,
    pub desktop: &'a str,
    pub keeping: Keeping,
}

/// **Whether a claim's runner keeps its work** (#2057).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keeping {
    /// Started cold with a volume of its own for its home: its checkout, conversation, profile
    /// sign-in, kept browser logins and pairing outlive its pod.
    Kept,
    /// A warm runner where the pool has one waiting, which keeps nothing past its pod.
    Fresh,
}

/// What purlis knows of a chat as it chooses a claim for it ([`kept_for`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChatOnARunner {
    /// A short task another chat dispatched, rather than a chat the person works in.
    pub dispatched: bool,
    /// How the chat's profile signs in to its harness, where it is known.
    pub sign_in: Option<crate::harness_declaration::SignInKind>,
    /// The person's "Keep this runner's work" switch, where they turned it one way or the
    /// other: it decides, whatever purlis would choose.
    pub keep_work: Option<bool>,
}

/// **purlis's choice of a kept or fresh claim** (spec #2041 stories 26 and 27): kept for a
/// chat the person works in and for any chat of a subscription profile, whose sign-in lives on
/// the kept volume; fresh for a short dispatched task. The person's "Keep this runner's work"
/// switch, where set, decides.
pub fn kept_for(chat: &ChatOnARunner) -> Keeping {
    use crate::harness_declaration::SignInKind;
    let kept = match chat.keep_work {
        Some(kept) => kept,
        None => chat.sign_in == Some(SignInKind::Subscription) || !chat.dispatched,
    };
    if kept { Keeping::Kept } else { Keeping::Fresh }
}

/// The Claim purlis makes: `kept` only on a kept one, so a fresh claim reads as every claim did
/// before claims were kept.
pub fn claim_definition(
    namespace: &str,
    name: &str,
    pool: &str,
    requester: &str,
    keeping: Keeping,
) -> serde_json::Value {
    let mut spec = serde_json::json!({ "pool": pool, "requester": requester });
    if keeping == Keeping::Kept {
        spec["kept"] = serde_json::Value::Bool(true);
    }
    serde_json::json!({
        "apiVersion": "runners.purlis.dev/v1alpha1",
        "kind": "Claim",
        "metadata": { "name": name, "namespace": namespace },
        "spec": spec,
    })
}

/// One claim this desktop made, as it knows it before the runner answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claimed {
    pub namespace: String,
    pub name: String,
    pub uid: String,
    pub requester: String,
}

/// A cluster runner this desktop paired: the claim, what the operator vouched for, and what
/// the runner answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paired {
    pub claimed: Claimed,
    pub vouched: Vouched,
    pub pinned: Pinned,
}

/// **The statement on `claimed`'s status, checked**: signed by the key of `pin`, made in its
/// cluster, and about exactly that claim and requester. `None` while the operator has written
/// none.
pub fn vouched_for(
    kubectl: &Kubectl,
    pin: &PoolPin,
    claimed: &Claimed,
) -> Result<Option<Vouched>, Refused> {
    let Some(signed) = kubectl.statement(&claimed.namespace, &claimed.name)? else {
        return Ok(None);
    };
    checked(pin, claimed, &signed).map(Some)
}

/// `signed`, verified with `pin` and held to `claimed`, now.
pub fn checked(pin: &PoolPin, claimed: &Claimed, signed: &Signed) -> Result<Vouched, Refused> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default();
    checked_at(pin, claimed, signed, now)
}

/// [`checked`], at `now` (seconds since the epoch): a statement past its `until`, or one that
/// claims to hold more than [`MOST_AHEAD`] from now, or made in another cluster than the pinned
/// one, is refused.
pub fn checked_at(
    pin: &PoolPin,
    claimed: &Claimed,
    signed: &Signed,
    now: u64,
) -> Result<Vouched, Refused> {
    let vouched = pin.verified(signed)?;
    if vouched.until <= now {
        return Err(Refused::NotVouched(format!(
            "the cluster operator's statement about the runner of claim {}/{} no longer holds: \
             it has not been signed again, so purlis refused it; is the cluster operator \
             running, or is this machine's clock ahead?",
            claimed.namespace, claimed.name
        )));
    }
    if vouched.until > now.saturating_add(MOST_AHEAD) {
        return Err(Refused::NotVouched(format!(
            "the cluster operator's statement about the runner of claim {}/{} says it holds for \
             more than fifteen minutes, which no statement does, so purlis refused it. This \
             machine's clock is probably behind the cluster's: set it right and try again",
            claimed.namespace, claimed.name
        )));
    }
    if !vouched.is_for(
        &claimed.namespace,
        &claimed.name,
        &claimed.uid,
        &claimed.requester,
    ) {
        return Err(Refused::NotVouched(format!(
            "the cluster operator's statement is about another claim or requester than the claim {}/{} \
             this desktop made as {}, or its runner is not in that requester's own namespace, so \
             purlis refused it",
            claimed.namespace, claimed.name, claimed.requester
        )));
    }
    Ok(vouched)
}

/// What a pairing answered, held to what the operator vouched for: the runner's key must be
/// the vouched key, or it is refused as a changed key.
pub fn agreed(claimed: &Claimed, vouched: &Vouched, answered: Pinned) -> Result<Pinned, Refused> {
    if answered.key != vouched.key {
        return Err(Refused::KeyChanged {
            claim: format!("{}/{}", claimed.namespace, claimed.name),
        });
    }
    Ok(answered)
}

/// **Pairs the runner the operator vouched for**, over this desktop's own `kubectl exec` into
/// the pod the statement names: the runner's device id and key, once its key is the one the
/// operator vouched for ([`agreed`]).
pub fn pair(
    kubectl: &Kubectl,
    claimed: &Claimed,
    vouched: &Vouched,
    desktop: &Pinned,
) -> Result<Paired, Refused> {
    let line = format!("{}\n", crate::runnerlink::to_line(desktop));
    let out = kubectl.run(
        &kubectl.pair_args(&vouched.runner_namespace, &vouched.pod),
        Some(&line),
        crate::runnerlink::MOST_LINE,
    )?;
    let answered = crate::runnerlink::from_line(&out).map_err(Refused::Other)?;
    let pinned = agreed(claimed, vouched, answered)?;
    Ok(Paired {
        claimed: claimed.clone(),
        vouched: vouched.clone(),
        pinned,
    })
}

/// How many times a Claim's repos are written, where another write came first.
const CLAIM_WRITES: u32 = 3;

/// How many times, a second apart, a Claim's status is read for where to ask, where its Pool opts in.
const MINT_READS: u32 = 5;

/// The most repos a Claim's `spec.forge.repos` holds: what its definition allows.
pub const MOST_CLAIM_REPOS: usize = 256;

/// `named` with `adding`, in order and each once, ignoring case as GitHub does; `None` where that
/// adds nothing.
pub fn repos_with(named: &[String], adding: &[String]) -> Option<Vec<String>> {
    let mut all = named.to_vec();
    for repo in adding {
        if !all.iter().any(|held| held.eq_ignore_ascii_case(repo)) {
            all.push(repo.clone());
        }
    }
    (all.len() > named.len()).then(|| {
        all.sort();
        all
    })
}

/// **What a runner is told of the port** (#4207): the address a Claim's status says, and the key
/// this desktop pinned, whatever key that status says. A status is nobody's signed word, so it can
/// only send a runner to an address that then fails its handshake. During a roll this desktop has
/// followed already, the runner's mints go to its desktop until the port shows that key.
pub fn told_of_the_port(
    claimed: &Claimed,
    pin: &PoolPin,
    address: String,
) -> crate::forge::mintport::Told {
    crate::forge::mintport::Told {
        namespace: claimed.namespace.clone(),
        claim: claimed.name.clone(),
        cluster: pin.cluster.clone(),
        address,
        keys: vec![pin.key.spelled()],
        left: Vec::new(),
    }
}

/// **Tells the runner of `paired` where the mint port is** (#4207), where the Claim's status says
/// ([`told_of_the_port`]), waiting a little where its Pool opts in and the status is yet to say. It
/// decides nothing: the runner goes on asking this desktop, with the person's Ask, wherever it was
/// not told.
pub fn tell_the_mint_port(
    kubectl: &Kubectl,
    pin: &PoolPin,
    pool: &str,
    paired: &Paired,
    every: std::time::Duration,
) -> Result<(), Refused> {
    let Claimed {
        namespace, name, ..
    } = &paired.claimed;
    let seen = kubectl.pool(pool)?;
    if !seen.mints_in_cluster {
        return Ok(());
    }
    let mut reads = MINT_READS;
    let (address, _says_of_its_key) = loop {
        reads -= 1;
        match kubectl.claim_mint(namespace, name)? {
            Some(said) => break said,
            None if reads == 0 => {
                return Err(Refused::Other(
                    "the claim's status does not say where to ask for a token".to_owned(),
                ));
            }
            None => std::thread::sleep(every),
        }
    };
    let told = told_of_the_port(&paired.claimed, pin, address);
    let line = serde_json::to_string(&told).map_err(|why| Refused::Other(why.to_string()))?;
    kubectl
        .run(
            &kubectl.tell_mint_port_args(&paired.vouched.runner_namespace, &paired.vouched.pod),
            Some(&format!("{line}\n")),
            crate::runnerlink::MOST_LINE,
        )
        .map(|_| ())
}

/// **What to tell the runner of `claimed` at a link** (#4330), from one read of its Pool and Claim:
/// `None` where the Pool does not opt in or the status is yet to say, so the next link asks again.
pub fn to_tell_at_a_link(
    seen: &PoolSeen,
    kubectl: &Kubectl,
    pin: &PoolPin,
    claimed: &Claimed,
) -> Result<Option<crate::forge::mintport::Told>, Refused> {
    if !seen.mints_in_cluster {
        return Ok(None);
    }
    Ok(kubectl
        .claim_mint(&claimed.namespace, &claimed.name)?
        .map(|(address, _says_of_its_key)| told_of_the_port(claimed, pin, address)))
}

/// What a runner prints once a word at a link settled it (#4330).
pub const TOLD_SETTLED: &str = "settled";

/// **Tells the runner in `pod` again** (#4330, [`crate::forge::mintport::tell_again`]): true where it
/// says that settled it, so this desktop need not say it again.
pub fn tell_again(
    kubectl: &Kubectl,
    runner_namespace: &str,
    pod: &str,
    told: &crate::forge::mintport::Told,
) -> Result<bool, Refused> {
    let line = serde_json::to_string(told).map_err(|why| Refused::Other(why.to_string()))?;
    kubectl
        .run(
            &kubectl.tell_mint_port_again_args(runner_namespace, pod),
            Some(&format!("{line}\n")),
            crate::runnerlink::MOST_LINE,
        )
        .map(|said| said.trim() == TOLD_SETTLED)
}

/// **Before every link**: the operator still vouches for the pod and key `paired` was paired
/// with. Another pod or key is refused as a changed key; no statement at all is refused too. A
/// kept claim's pod made again under its name, answering the key it was paired with, is the
/// same runner (#2057): its pairing is the claim's.
pub fn still_vouched(kubectl: &Kubectl, pin: &PoolPin, paired: &Paired) -> Result<(), Refused> {
    still_vouched_for(
        kubectl,
        pin,
        &paired.claimed,
        &paired.vouched.pod,
        &paired.pinned,
    )
}

/// [`still_vouched`], for the runner of `claimed` paired in `pod` as `pinned`: what a desktop
/// keeps of a pairing (`runners.json`, #3292).
pub fn still_vouched_for(
    kubectl: &Kubectl,
    pin: &PoolPin,
    claimed: &Claimed,
    pod: &str,
    pinned: &Pinned,
) -> Result<(), Refused> {
    let now = vouched_for(kubectl, pin, claimed)?.ok_or_else(|| {
        Refused::NotVouched(format!(
            "the cluster operator no longer vouches for the runner of claim {}/{}",
            claimed.namespace, claimed.name
        ))
    })?;
    held_to_pod(claimed, pod, pinned, &now)
}

/// `now`, what the operator vouches for, held to what `paired` was paired with.
pub fn held_to(paired: &Paired, now: &Vouched) -> Result<(), Refused> {
    held_to_pod(&paired.claimed, &paired.vouched.pod, &paired.pinned, now)
}

/// `now` held to a pairing kept as `claimed`'s runner in `pod`, answering `pinned`: in that
/// requester's own runner namespace (#2055), the same pod and the same key.
fn held_to_pod(
    claimed: &Claimed,
    pod: &str,
    pinned: &Pinned,
    now: &Vouched,
) -> Result<(), Refused> {
    if now.runner_namespace != runner_namespace(&claimed.requester)
        || now.pod != pod
        || now.key != pinned.key
    {
        return Err(Refused::KeyChanged {
            claim: format!("{}/{}", claimed.namespace, claimed.name),
        });
    }
    Ok(())
}

/// **Before every link to a cluster runner** (#3292), at `now`: the Pool `pool`'s status read,
/// `pin` follows a roll its pinned key vouches for ([`PoolPin::follow`], #3305), in place, so the
/// caller keeps it where it keeps the pool's pin once it differs from what it was; a Pool that
/// names another cluster, or a key the pin neither is nor vouched for, is
/// [`Refused::OperatorKeyChanged`]. Then the operator must still vouch, with the pin as it now
/// is, for the pod and key the runner was paired with ([`still_vouched_for`]).
///
/// **Right after a pin moves**, a Claim the operator has not reconciled since still carries its
/// statement signed by the old key alone, with no `nextSignature`: that is
/// [`Refused::NotVouched`], which refuses this link and no more. The next try, once the operator
/// has signed it again, links; nothing is unpaired for it.
pub fn before_link(
    kubectl: &Kubectl,
    pin: &mut PoolPin,
    pool: &str,
    claimed: &Claimed,
    pod: &str,
    pinned: &Pinned,
    now: u64,
) -> Result<(), Refused> {
    let seen = kubectl.pool(pool)?;
    pin.follow(&seen, now)
        .map_err(|_| Refused::OperatorKeyChanged {
            pool: pool.to_owned(),
        })?;
    still_vouched_for(kubectl, pin, claimed, pod, pinned)
}

/// **Claims a kept runner of `pool` as `name` in `namespace`, and pairs it with `desktop`**
/// (#3292): made as who this kubeconfig is ([`Kubectl::whoami`]), waited on for at most
/// `patience` until the operator vouches for its key (asked every `every`), then paired over this
/// desktop's own `kubectl exec` ([`pair`]). What fails deletes the claim it made, and says what
/// the claim last said of itself.
pub fn claim_and_pair(
    kubectl: &Kubectl,
    pin: &PoolPin,
    to: &ClaimTo<'_>,
    desktop: &Pinned,
    patience: std::time::Duration,
    every: std::time::Duration,
) -> Result<Paired, Refused> {
    let ClaimTo {
        pool,
        namespace,
        name,
        desktop: label,
        keeping,
    } = *to;
    // Claimed by name: a pool's name and namespace, each as Kubernetes names them.
    if !is_subdomain(pool) || !is_label(namespace) || !is_subdomain(name) {
        return Err(Refused::Other(format!(
            "purlis claims a runner only by Kubernetes names, and {pool}, {namespace} or {name} \
             is not one"
        )));
    }
    let requester = kubectl.whoami()?;
    let uid = kubectl.claim_marked(namespace, name, pool, &requester, keeping, Some(label))?;
    let claimed = Claimed {
        namespace: namespace.to_owned(),
        name: name.to_owned(),
        uid,
        requester,
    };
    let paired = vouched_within(kubectl, pin, &claimed, patience, every)
        .and_then(|vouched| pair(kubectl, &claimed, &vouched, desktop));
    if let Ok(paired) = &paired
        && let Err(why) = tell_the_mint_port(
            kubectl,
            pin,
            pool,
            paired,
            std::time::Duration::from_secs(1),
        )
    {
        tracing::warn!(
            "purlis: the runner of pool {pool} will ask this desktop for its tokens: {why}"
        );
    }
    paired.map_err(|why| match kubectl.delete_claim(namespace, name) {
        Ok(()) => why,
        Err(left) => {
            let said = format!(
                "{why}; and the claim could not be deleted ({left}): delete the claim {name} in \
                 namespace {namespace} with kubectl"
            );
            // The wait still ended it, so a longer one is still offered.
            if matches!(why, Refused::WaitRanOut(_)) {
                Refused::WaitRanOut(said)
            } else {
                Refused::Other(said)
            }
        }
    })
}

/// The statement on `claimed`, once the operator has written one, asked every `every` for at
/// most `patience`.
fn vouched_within(
    kubectl: &Kubectl,
    pin: &PoolPin,
    claimed: &Claimed,
    patience: std::time::Duration,
    every: std::time::Duration,
) -> Result<Vouched, Refused> {
    let start = std::time::Instant::now();
    loop {
        if let Some(vouched) = vouched_for(kubectl, pin, claimed)? {
            return Ok(vouched);
        }
        if start.elapsed() >= patience {
            let said = kubectl
                .claim_said(&claimed.namespace, &claimed.name)
                .map(|said| format!(" The claim says: {said}."))
                .unwrap_or_default();
            return Err(Refused::WaitRanOut(format!(
                "the cluster operator did not vouch for the runner of claim {}/{} within {} \
                 seconds, so purlis deleted the claim.{said}",
                claimed.namespace,
                claimed.name,
                patience.as_secs()
            )));
        }
        std::thread::sleep(every);
    }
}

/// What a desktop writes on a Claim to run its idle runner again (#3980):
/// `operator/src/lifecycle.rs` reads it.
pub const RUN_AGAIN_ANNOTATION: &str = "runners.purlis.dev/run-again";

/// When the Claim `held` says its runner was suspended for being idle, where it says so.
pub fn idle_since(held: &serde_json::Value) -> Option<&str> {
    held.pointer("/status/conditions")?
        .as_array()?
        .iter()
        .find(|one| {
            one.get("type").and_then(serde_json::Value::as_str) == Some("Idle")
                && one.get("status").and_then(serde_json::Value::as_str) == Some("True")
        })?
        .get("lastTransitionTime")?
        .as_str()
}

/// The word its desktop wrote on the Claim `held` to run its idle runner again, where it did.
fn run_again_word(held: &serde_json::Value) -> Option<&str> {
    held.pointer("/metadata/annotations")?
        .get(RUN_AGAIN_ANNOTATION)?
        .as_str()
}

/// **Whether the Claim `held`'s runner is suspended for being idle and nothing else** (#4403), so
/// the word to run it again brings it back and a start waits for it. An `Idle` kept while `Ready`
/// says something else, an object in the way or its pool deleted, is not acted on, however long a
/// start waited. The word itself says nothing later: it stays on the Claim after the runner runs.
pub fn suspended_for_being_idle(held: &serde_json::Value) -> bool {
    idle_since(held).is_some()
        && held
            .pointer("/status/conditions")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|conditions| {
                conditions.iter().any(|one| {
                    one.get("type").and_then(serde_json::Value::as_str) == Some("Ready")
                        && one.get("reason").and_then(serde_json::Value::as_str) == Some("Idle")
                })
            })
}

/// **Where a runner on its way has got to** (#3709), in the words Pool setup uses: no Claim, pod or
/// node, for a reader who has never heard of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerStage {
    pub said: &'static str,
    pub more: &'static str,
    /// The cluster's own words for why it waits, where it gave them.
    pub cluster_said: Option<String>,
}

/// What a stage says where the cluster gave a reason for the wait.
const STUCK: &str = "This is taking a while. The cluster says why below. Your admin can act on it.";

/// **The stage the Claim `held` says its runner is at**: `Ready` is `False` with the reason the
/// operator gives while a cold claim starts, or `Idle` for one that was resting. Any other reason
/// is not a stage, and the window says what it says of any start.
pub fn stage_of(held: &serde_json::Value) -> Option<RunnerStage> {
    let ready = held
        .pointer("/status/conditions")?
        .as_array()?
        .iter()
        .find(|one| one.get("type").and_then(serde_json::Value::as_str) == Some("Ready"))?;
    let field = |name: &str| ready.get(name).and_then(serde_json::Value::as_str);
    if field("status") != Some("False") {
        return None;
    }
    // Only what the cluster said is wrong: the operator words every stage for kubectl, and those
    // words are no reason to worry. Whole here: what shows it masks and cuts it.
    let message = field("message").unwrap_or_default();
    let why = |after: &str| {
        message
            .split_once(after)
            .map(|(_, why)| why.trim().to_owned())
            .filter(|why| !why.is_empty())
    };
    let (said, more, cluster_said) = match field("reason")? {
        "Scheduling" => (
            "Waiting for room in the cluster",
            "The cluster has no machine free for your runner yet. You can keep working; this \
             chat starts by itself.",
            why(" The scheduler: "),
        ),
        "Pulling" => (
            "Downloading the runner image",
            "Only the first runner on a machine needs this. It can take a few minutes.",
            why("is not pulled yet: "),
        ),
        "Starting" => ("Starting your runner", "Almost there.", None),
        "Idle" => (
            "Your runner was resting, and is coming back",
            "It rests when nothing runs on it. Your work is still there.",
            None,
        ),
        _ => return None,
    };
    let more = match cluster_said {
        Some(_) => STUCK,
        None => more,
    };
    Some(RunnerStage {
        said,
        more,
        cluster_said,
    })
}

/// What the Claim `held`'s `DeletionNotice` condition says, where it is `True` (#3981).
pub fn deletion_notice_of(held: &serde_json::Value) -> Option<String> {
    let notice = held
        .pointer("/status/conditions")?
        .as_array()?
        .iter()
        .find(|one| {
            one.get("type").and_then(serde_json::Value::as_str) == Some("DeletionNotice")
                && one.get("status").and_then(serde_json::Value::as_str) == Some("True")
        })?;
    notice
        .get("message")
        .and_then(serde_json::Value::as_str)
        .map(crate::shown::short)
}

pub mod idle;

#[cfg(test)]
pub(crate) mod tests;
