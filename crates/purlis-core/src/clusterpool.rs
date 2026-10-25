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
//!   The desktop pinned that key when the pool was added ([`OperatorKey`]), from the Pool's
//!   status, so a statement signed by any other key is refused.
//! - **The desktop pairs over its own `kubectl exec`** ([`pair`]): the runner answers with its
//!   device id and public key, as a Docker pool's does. A key that is not the one the operator
//!   vouched for is refused ([`Refused::KeyChanged`]), and so is a statement that no longer
//!   names the pod and key the desktop paired with, before every link ([`still_vouched`]).

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
    /// No statement this desktop can trust vouches for the runner: none yet, one signed by
    /// another key than the pool's, or one about another claim or requester.
    NotVouched(String),
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
            Refused::NotVouched(why) | Refused::Other(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for Refused {}

/// The first line of every statement: what it is, and its version.
pub const STATEMENT_HEADER: &str = "purlis runner link key v1";

/// **What the cluster operator vouches for**: one claim's runner, the pod it runs in, the
/// public link key that pod made as it started, and who asked for the claim. Signed as its
/// text ([`Vouched::text`]), one `name: value` line each, in this order, after
/// [`STATEMENT_HEADER`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vouched {
    pub namespace: String,
    pub claim: String,
    pub claim_uid: String,
    pub pod: String,
    pub pod_uid: String,
    pub key: PublicKey,
    pub requester: String,
}

/// The names of a statement's lines, in their order.
const LINES: [&str; 7] = [
    "namespace",
    "claim",
    "claim uid",
    "pod",
    "pod uid",
    "key",
    "requester",
];

/// The longest value a statement's line holds.
const MOST_VALUE: usize = 512;

impl Vouched {
    /// Its text, exactly as the operator signs it.
    pub fn text(&self) -> String {
        let key = hex_of(self.key.as_bytes());
        let values = [
            self.namespace.as_str(),
            &self.claim,
            &self.claim_uid,
            &self.pod,
            &self.pod_uid,
            &key,
            &self.requester,
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
        let bad = || Refused::NotVouched("the operator's statement is not one purlis reads".into());
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
        let key = bytes_of(&values[5]).ok_or_else(bad)?;
        let [namespace, claim, claim_uid, pod, pod_uid, _key, requester]: [String; 7] =
            values.try_into().map_err(|_| bad())?;
        Ok(Vouched {
            namespace,
            claim,
            claim_uid,
            pod,
            pod_uid,
            key: PublicKey::from_bytes(key),
            requester,
        })
    }

    /// Whether it is about the claim `namespace`/`claim` of uid `claim_uid`, asked for by
    /// `requester`: the one this desktop made.
    pub fn is_for(&self, namespace: &str, claim: &str, claim_uid: &str, requester: &str) -> bool {
        self.namespace == namespace
            && self.claim == claim
            && self.claim_uid == claim_uid
            && self.requester == requester
    }
}

/// Whether `value` can be a statement's value: not empty, at most [`MOST_VALUE`] bytes, and
/// nothing a line break or a control character could hide behind.
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
        let signed = signature_bytes(signature).ok_or_else(|| {
            Refused::NotVouched("the operator's signature is not one purlis reads".into())
        })?;
        ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, self.0)
            .verify(text.as_bytes(), &signed)
            .map_err(|_| {
                Refused::NotVouched(
                    "the statement about this runner is not signed by the cluster operator's \
                     key this desktop pinned when the pool was added, so purlis refused it"
                        .into(),
                )
            })?;
        Vouched::from_text(text)
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
#[derive(Debug, Clone)]
pub struct Kubectl {
    program: PathBuf,
    kubeconfig: Option<PathBuf>,
    context: Option<String>,
}

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
        }
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
            pod,
            "--container",
            RUNNER_CONTAINER,
            "--",
            PURLIS,
            "bridge",
        ])
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

    /// Runs kubectl with `args`, handing it `input`, and answers what it printed, up to `most`
    /// bytes. What it says on standard error is only shown, never read to decide anything.
    pub fn run(&self, args: &[String], input: Option<&str>, most: u64) -> Result<String, Refused> {
        use std::io::{Read, Write};
        let mut command = std::process::Command::new(&self.program);
        command
            .args(args)
            .stdin(if input.is_some() {
                std::process::Stdio::piped()
            } else {
                std::process::Stdio::null()
            })
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let what = || {
            args.iter()
                .find(|arg| !arg.starts_with('-') && !self.pointed().contains(arg))
                .cloned()
                .unwrap_or_default()
        };
        let mut child = crate::forklock::spawn(&mut command).map_err(|why| {
            Refused::Other(format!(
                "{} could not be run: {why}",
                self.program.display()
            ))
        })?;
        if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
            let _ = stdin.write_all(input.as_bytes());
        }
        let said = child.stderr.take().map(|stderr| {
            std::thread::spawn(move || {
                let mut kept = Vec::new();
                let mut stderr = stderr;
                let _ = (&mut stderr).take(MOST_SAID).read_to_end(&mut kept);
                let _ = std::io::copy(&mut stderr, &mut std::io::sink());
                kept
            })
        });
        let mut out = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            let _ = stdout.take(most + 1).read_to_end(&mut out);
        }
        if out.len() as u64 > most {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Refused::Other(format!(
                "kubectl {} printed more than the {most} bytes purlis reads of it, so it was ended",
                what()
            )));
        }
        let status = child.wait().map_err(|why| {
            Refused::Other(format!("{} did not finish: {why}", self.program.display()))
        })?;
        let said = said.and_then(|said| said.join().ok()).unwrap_or_default();
        if !status.success() {
            return Err(Refused::Other(format!(
                "kubectl {} failed: {}",
                what(),
                String::from_utf8_lossy(&said).trim()
            )));
        }
        String::from_utf8(out)
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

    /// **The operator's key a pool's status names**, for the desktop to pin as it adds the pool.
    pub fn operator_key(&self, pool: &str) -> Result<OperatorKey, Refused> {
        let held = self.json(
            &["get", "pools.runners.purlis.dev", pool, "--output", "json"],
            None,
        )?;
        let spelled = held
            .pointer("/status/signingKey")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                Refused::NotVouched(format!(
                    "the pool {pool} names no cluster operator's key yet: is purlis's cluster \
                     operator running there?"
                ))
            })?;
        OperatorKey::from_spelled(spelled)
    }

    /// **Claims a runner of `pool` in `namespace`**, as `requester` (who this kubeconfig is,
    /// [`Kubectl::whoami`]): the Claim's uid, which the operator's statement must name.
    pub fn claim(
        &self,
        namespace: &str,
        name: &str,
        pool: &str,
        requester: &str,
    ) -> Result<String, Refused> {
        let claim = serde_json::json!({
            "apiVersion": "runners.purlis.dev/v1alpha1",
            "kind": "Claim",
            "metadata": { "name": name, "namespace": namespace },
            "spec": { "pool": pool, "requester": requester },
        });
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

    /// The statement and signature on the Claim `namespace`/`name`'s status, where the operator
    /// has written one.
    pub fn statement(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<Option<(String, String)>, Refused> {
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
        let field = |name: &str| {
            held.pointer(&format!("/status/linkKey/{name}"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        Ok(field("statement").zip(field("signature")))
    }

    /// Deletes the Claim `namespace`/`name`, and with it its runner.
    pub fn delete_claim(&self, namespace: &str, name: &str) -> Result<(), Refused> {
        self.run(
            &self.args(&[
                "delete",
                "claims.runners.purlis.dev",
                name,
                "--namespace",
                namespace,
                "--wait=false",
            ]),
            None,
            MOST_PRINTED,
        )
        .map(|_| ())
    }
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

/// **The statement on `claimed`'s status, checked**: signed by `operator`, and about exactly
/// that claim and requester. `None` while the operator has written none.
pub fn vouched_for(
    kubectl: &Kubectl,
    operator: &OperatorKey,
    claimed: &Claimed,
) -> Result<Option<Vouched>, Refused> {
    let Some((text, signature)) = kubectl.statement(&claimed.namespace, &claimed.name)? else {
        return Ok(None);
    };
    checked(operator, claimed, &text, &signature).map(Some)
}

/// `text` signed as `signature`, verified with `operator` and held to `claimed`.
pub fn checked(
    operator: &OperatorKey,
    claimed: &Claimed,
    text: &str,
    signature: &str,
) -> Result<Vouched, Refused> {
    let vouched = operator.verified(text, signature)?;
    if !vouched.is_for(
        &claimed.namespace,
        &claimed.name,
        &claimed.uid,
        &claimed.requester,
    ) {
        return Err(Refused::NotVouched(format!(
            "the operator's statement is about another claim or requester than the claim {}/{} \
             this desktop made as {}, so purlis refused it",
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
        &kubectl.pair_args(&claimed.namespace, &vouched.pod),
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

/// **Before every link**: the operator still vouches for the pod and key `paired` was paired
/// with. Another pod or key is refused as a changed key; no statement at all is refused too.
pub fn still_vouched(
    kubectl: &Kubectl,
    operator: &OperatorKey,
    paired: &Paired,
) -> Result<(), Refused> {
    let now = vouched_for(kubectl, operator, &paired.claimed)?.ok_or_else(|| {
        Refused::NotVouched(format!(
            "the cluster operator no longer vouches for the runner of claim {}/{}",
            paired.claimed.namespace, paired.claimed.name
        ))
    })?;
    held_to(paired, &now)
}

/// `now`, what the operator vouches for, held to what `paired` was paired with.
pub fn held_to(paired: &Paired, now: &Vouched) -> Result<(), Refused> {
    if now.pod_uid != paired.vouched.pod_uid || now.key != paired.pinned.key {
        return Err(Refused::KeyChanged {
            claim: format!("{}/{}", paired.claimed.namespace, paired.claimed.name),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
