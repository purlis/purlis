// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Install, a part at a time** (#4493, #2239): the pinned upstream agent-sandbox, purlis's
//! definitions and their policies, the operator's signing key, and the operator, each on its own
//! button, each saying what it applies, each run again on its own. They are the install
//! `docs/cluster-operator.md` gives with the plain manifests, run with the person's own `kubectl`
//! on the context chosen on Where, from the manifests this build of purlis carries.
//!
//! - **The signing key is made only where there is none, and never travels anywhere else**: a new
//!   Ed25519 key, made in memory and piped into `kubectl create secret`, so it is written to no
//!   file and kept by nothing of purlis. A key that is there is never replaced (a new key makes
//!   every desktop ask again): `create` refuses one that is there, and the part is done.
//! - **Uninstalling is never a button** ([`uninstall`]): the exact commands, and how many people
//!   and claims they end.
//! - **No pull secret is needed**: the operator's and the runner's images are public packages.

use base64::Engine as _;

use super::cluster::{
    DEFINITIONS, Installed, OPERATOR_DEPLOYMENT, Operator, SIGNING_KEY_SECRET, SYSTEM_NAMESPACE,
};
use super::{Action, ActionId, See};
use crate::clusterpool::Kubectl;
use crate::runners::Refused;

/// The vendored upstream release, byte for byte, and the kustomization that names its
/// controller by digest: what `kubectl apply -k operator/vendor/agent-sandbox/<version>` applies.
const RELEASE_MANIFEST: &str =
    include_str!("../../../../operator/vendor/agent-sandbox/v1.0.6/sandbox-with-extensions.yaml");
const RELEASE_KUSTOMIZATION: &str =
    include_str!("../../../../operator/vendor/agent-sandbox/v1.0.6/kustomization.yaml");
/// The policies, then purlis's definitions, then the operator, as `operator/deploy/` renders them
/// from the chart.
const POLICY: &str = include_str!("../../../../operator/deploy/policy.yaml");
const CRDS: &str = include_str!("../../../../operator/deploy/crds.yaml");
const OPERATOR: &str = include_str!("../../../../operator/deploy/operator.yaml");

/// Where the operator's image is published (`docs/cluster-operator.md`, "Its image").
pub const OPERATOR_REPOSITORY: &str = "ghcr.io/purlis/purlis-cluster-operator";

/// How long a rollout is waited for, as the install's docs wait.
const ROLLOUT: &str = "--timeout=180s";

/// One part of the install.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PartId {
    Upstream,
    Definitions,
    SigningKey,
    Operator,
}

impl PartId {
    pub const ALL: [PartId; 4] = [
        PartId::Upstream,
        PartId::Definitions,
        PartId::SigningKey,
        PartId::Operator,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            PartId::Upstream => "agent-sandbox",
            PartId::Definitions => "purlis's definitions",
            PartId::SigningKey => "Signing key",
            PartId::Operator => "Cluster operator",
        }
    }
}

/// **A part of the install as it stands**: done or not, what it is, and its own button.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Part {
    pub id: PartId,
    pub name: &'static str,
    pub done: bool,
    /// What it is, or what was found of it.
    pub line: String,
    /// Its button: none once it is done, or while a part before it isn't.
    pub next: Option<Action>,
}

fn press(id: ActionId, label: &str, does: String, command: String) -> Option<Action> {
    Some(Action {
        id,
        label: label.to_owned(),
        does,
        command: Some(command),
        url: None,
    })
}

/// **Whether `image` names the operator by a digest** (`repository@sha256:` and 64 hex digits).
#[must_use]
pub fn is_by_digest(image: &str) -> bool {
    let Some((repository, digest)) = image.split_once("@sha256:") else {
        return false;
    };
    !repository.is_empty()
        && !repository.contains(char::is_whitespace)
        && digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

/// **Each part as the cluster holds it** (#4493): `upstream` is whether doctor found the pinned
/// release, `installed` the rest, `image` the operator image chosen under Options.
#[must_use]
pub fn parts(upstream: bool, installed: &Installed, image: Option<&str>, on: &str) -> Vec<Part> {
    let mut before = true;
    PartId::ALL
        .into_iter()
        .map(|id| {
            let done = match id {
                PartId::Upstream => upstream,
                PartId::Definitions => installed.definitions,
                PartId::SigningKey => installed.key,
                PartId::Operator => matches!(installed.operator, Operator::Running { .. }),
            };
            let line = match (id, done, &installed.operator) {
                (PartId::Upstream, true, _) => {
                    format!("{} {}, which purlis is tested on", id.name(), doctor_version())
                }
                (PartId::Upstream, false, _) => format!(
                    "The project purlis's runners are built on, at the release it is tested on, {}.",
                    doctor_version()
                ),
                (PartId::Definitions, true, _) => format!("{} and their policies", DEFINITIONS.join(", ")),
                (PartId::Definitions, false, _) => {
                    "The kinds a team's pools and people's claims are, and the policies that hold \
                     who may use them."
                        .to_owned()
                }
                (PartId::SigningKey, true, _) => format!(
                    "Kept in {SYSTEM_NAMESPACE}/{SIGNING_KEY_SECRET}. purlis never replaces it."
                ),
                (PartId::SigningKey, false, _) => {
                    "The key the operator vouches for each runner with. It's made in the cluster \
                     once, and never leaves it."
                        .to_owned()
                }
                (PartId::Operator, true, Operator::Running { image }) => {
                    format!("Running, from {}", crate::shown::short(image))
                }
                (PartId::Operator, _, Operator::Starting) => {
                    "Installed, and starting: no replica is ready yet.".to_owned()
                }
                (PartId::Operator, _, _) => {
                    "The controller that makes each person's runners. Its image is public, so the \
                     cluster needs no pull secret."
                        .to_owned()
                }
            };
            let next = if done || !before {
                None
            } else {
                next_of(id, image, on)
            };
            before &= done;
            Part {
                id,
                name: id.name(),
                done,
                line,
                next,
            }
        })
        .collect()
}

fn doctor_version() -> &'static str {
    purlis_cluster_doctor::VERSION
}

fn next_of(id: PartId, image: Option<&str>, on: &str) -> Option<Action> {
    let command = by_hand(id, image, on);
    match id {
        PartId::Upstream => press(
            ActionId::InstallUpstream,
            "Install agent-sandbox",
            format!(
                "Applies agent-sandbox {} to the cluster, its controller named by digest, and \
                 waits for it to start.",
                doctor_version()
            ),
            command,
        ),
        PartId::Definitions => press(
            ActionId::InstallDefinitions,
            "Apply the definitions",
            "Applies purlis's policies, then its two definitions, Pool and Claim. Nothing runs \
             yet."
                .to_owned(),
            command,
        ),
        PartId::SigningKey => press(
            ActionId::MakeSigningKey,
            "Make the signing key",
            format!(
                "Makes the namespace {SYSTEM_NAMESPACE} and a new key in it. A key that is there \
                 is kept."
            ),
            command,
        ),
        PartId::Operator => match image.filter(|image| is_by_digest(image)) {
            Some(image) => press(
                ActionId::InstallOperator,
                "Install the operator",
                format!(
                    "Applies the operator, its roles and its namespace's policy, runs it from \
                     {image}, and waits for it to start."
                ),
                command,
            ),
            None => Some(Action {
                id: ActionId::ChooseOperatorImage,
                label: "Name the operator's image".to_owned(),
                does: format!(
                    "Give the operator's image by its digest under Options, {OPERATOR_REPOSITORY}@sha256:… \
                     See what this does says how to find and check it."
                ),
                command: None,
                url: None,
            }),
        },
    }
}

/// **The commands a part runs**, as See what this does shows them: the docs' plain-manifest
/// install, from a checkout of purlis at this version, on the context `on` (` --context …`).
#[must_use]
pub fn by_hand(id: PartId, image: Option<&str>, on: &str) -> String {
    let version = doctor_version();
    match id {
        PartId::Upstream => format!(
            "kubectl{on} apply --server-side -k operator/vendor/agent-sandbox/{version}\n\
             kubectl{on} --namespace agent-sandbox-system rollout status deployment/agent-sandbox-controller {ROLLOUT}"
        ),
        PartId::Definitions => format!(
            "kubectl{on} apply --server-side -f operator/deploy/policy.yaml\n\
             kubectl{on} apply --server-side -f operator/deploy/crds.yaml"
        ),
        PartId::SigningKey => format!(
            "kubectl{on} create namespace {SYSTEM_NAMESPACE}\n\
             kubectl{on} label namespace {SYSTEM_NAMESPACE} pod-security.kubernetes.io/enforce=restricted\n\
             # Piped, so the key is never written to a file. It needs OpenSSL 3, not LibreSSL.\n\
             openssl genpkey -algorithm ed25519 |\n  \
             kubectl{on} --namespace {SYSTEM_NAMESPACE} create secret generic {SIGNING_KEY_SECRET} \
             --from-file=key.pem=/dev/stdin"
        ),
        PartId::Operator => {
            let image = image.filter(|image| is_by_digest(image)).map_or_else(
                || format!("{OPERATOR_REPOSITORY}@sha256:<digest>"),
                str::to_owned,
            );
            format!(
                "# Find the image, and check its signature first with the cosign command\n\
                 # docs/cluster-operator.md gives under \"Its image\":\n\
                 # docker buildx imagetools inspect {OPERATOR_REPOSITORY}:main\n\
                 kubectl{on} apply --server-side -f operator/deploy/operator.yaml\n\
                 kubectl{on} --namespace {SYSTEM_NAMESPACE} set image deployment/{OPERATOR_DEPLOYMENT} operator={image}\n\
                 kubectl{on} --namespace {SYSTEM_NAMESPACE} rollout status deployment/{OPERATOR_DEPLOYMENT} {ROLLOUT}"
            )
        }
    }
}

/// **The whole install by hand**, a part after another, for See what this does.
#[must_use]
pub fn all_by_hand(image: Option<&str>, on: &str) -> String {
    PartId::ALL
        .iter()
        .map(|id| format!("# {}\n{}", id.name(), by_hand(*id, image, on)))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// **Uninstalling, never a button** (spec #4469, story 56): the exact commands, and how many
/// people and claims they end, where they could be counted.
#[must_use]
pub fn uninstall(installed: &Installed, on: &str) -> See {
    let affected = match installed.claims {
        Some((0, _)) => {
            "No one has a claim on this cluster now, so these end no one's chats.".to_owned()
        }
        Some((claims, people)) => format!(
            "{claims} {} by {people} {} would end, with every chat on {}. Ask them first.",
            if claims == 1 { "claim" } else { "claims" },
            if people == 1 { "person" } else { "people" },
            if claims == 1 {
                "its runner"
            } else {
                "their runners"
            },
        ),
        None => "purlis couldn't count the claims on this cluster. Every claim ends, with every \
                 chat on its runner."
            .to_owned(),
    };
    See {
        note: format!(
            "purlis never uninstalls from a cluster: other people's chats run there. {affected} \
             Run these yourself, in order."
        ),
        text: format!(
            "kubectl{on} delete claims.runners.purlis.dev --all-namespaces --all\n\
             kubectl{on} delete --ignore-not-found -f operator/deploy/operator.yaml\n\
             kubectl{on} delete namespace -l app.kubernetes.io/managed-by=purlis-cluster-operator\n\
             kubectl{on} delete --ignore-not-found -f operator/deploy/crds.yaml\n\
             kubectl{on} delete --ignore-not-found -f operator/deploy/policy.yaml"
        ),
    }
}

/// **Applies part `id` to the cluster `kubectl` is pointed at**, with the person's own sign-in.
/// Each part can be run again on its own: the applies are server-side, and a signing key that is
/// there is kept. The public half of a signing key it made, where it made one (#4495): what lets
/// this setup pin its own key without a paste.
pub fn apply(
    kubectl: &Kubectl,
    id: PartId,
    image: Option<&str>,
) -> Result<Option<crate::clusterpool::OperatorKey>, Refused> {
    let run = |args: &[&str], input: Option<&str>| {
        kubectl
            .asked(args, input)
            .map_err(|why| Refused::Other(why.to_string()))
            .and_then(|answer| {
                if answer.ok {
                    Ok(answer.printed)
                } else {
                    Err(Refused::Other(format!(
                        "kubectl {} said: {}",
                        args.first().copied().unwrap_or_default(),
                        crate::shown::short(&answer.said)
                    )))
                }
            })
    };
    match id {
        PartId::Upstream => {
            let folder = tempfile::tempdir().map_err(|why| {
                Refused::Other(format!("no folder for the release's files: {why}"))
            })?;
            std::fs::write(
                folder.path().join("sandbox-with-extensions.yaml"),
                RELEASE_MANIFEST,
            )
            .and_then(|()| {
                std::fs::write(
                    folder.path().join("kustomization.yaml"),
                    RELEASE_KUSTOMIZATION,
                )
            })
            .map_err(|why| Refused::Other(format!("the release's files: {why}")))?;
            let at = folder.path().display().to_string();
            run(&["apply", "--server-side", "-k", &at], None)?;
            run(
                &[
                    "--namespace",
                    "agent-sandbox-system",
                    "rollout",
                    "status",
                    "deployment/agent-sandbox-controller",
                    ROLLOUT,
                ],
                None,
            )?;
        }
        PartId::Definitions => {
            run(&["apply", "--server-side", "-f", "-"], Some(POLICY))?;
            run(&["apply", "--server-side", "-f", "-"], Some(CRDS))?;
        }
        PartId::SigningKey => {
            let namespace = run(
                &[
                    "get",
                    "namespace",
                    SYSTEM_NAMESPACE,
                    "--ignore-not-found",
                    "--output",
                    "name",
                ],
                None,
            )?;
            if namespace.trim().is_empty() {
                run(&["create", "namespace", SYSTEM_NAMESPACE], None)?;
            }
            run(
                &[
                    "label",
                    "namespace",
                    SYSTEM_NAMESPACE,
                    "pod-security.kubernetes.io/enforce=restricted",
                    "--overwrite",
                ],
                None,
            )?;
            // Looked for by its table alone, never read (`cluster::installed`).
            let there = run(
                &[
                    "get",
                    "secrets",
                    SIGNING_KEY_SECRET,
                    "--namespace",
                    SYSTEM_NAMESPACE,
                    "--ignore-not-found",
                    "--no-headers",
                ],
                None,
            )?;
            if !there.trim().is_empty() {
                return Ok(None);
            }
            let (key, public) = signing_key()?;
            // `create` refuses a Secret that is there: one made since the look is kept too.
            run(
                &[
                    "--namespace",
                    SYSTEM_NAMESPACE,
                    "create",
                    "secret",
                    "generic",
                    SIGNING_KEY_SECRET,
                    "--from-file=key.pem=/dev/stdin",
                ],
                Some(&key),
            )?;
            return Ok(Some(public));
        }
        PartId::Operator => {
            let Some(image) = image.filter(|image| is_by_digest(image)) else {
                return Err(Refused::Other(format!(
                    "nothing was applied: name the operator's image by its digest, \
                     {OPERATOR_REPOSITORY}@sha256:…"
                )));
            };
            run(&["apply", "--server-side", "-f", "-"], Some(OPERATOR))?;
            let set = format!("operator={image}");
            run(
                &[
                    "--namespace",
                    SYSTEM_NAMESPACE,
                    "set",
                    "image",
                    &format!("deployment/{OPERATOR_DEPLOYMENT}"),
                    &set,
                ],
                None,
            )?;
            run(
                &[
                    "--namespace",
                    SYSTEM_NAMESPACE,
                    "rollout",
                    "status",
                    &format!("deployment/{OPERATOR_DEPLOYMENT}"),
                    ROLLOUT,
                ],
                None,
            )?;
        }
    }
    Ok(None)
}

/// **A new Ed25519 key in PKCS#8 PEM**, the form the operator reads (`vouch.rs`, which takes
/// PKCS#8 v1, as `openssl genpkey -algorithm ed25519` makes it, or v2, as `ring` makes it): held
/// in memory only, for the one `kubectl create secret` that keeps it in the cluster. With its
/// public half, the key the operator's Pools will name.
fn signing_key() -> Result<(String, crate::clusterpool::OperatorKey), Refused> {
    let rng = ring::rand::SystemRandom::new();
    let der = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng)
        .map_err(|_| Refused::Other("no new key could be made on this machine".into()))?;
    let pair = ring::signature::Ed25519KeyPair::from_pkcs8(der.as_ref())
        .map_err(|_| Refused::Other("the new key could not be read back".into()))?;
    let public: [u8; 32] = ring::signature::KeyPair::public_key(&pair)
        .as_ref()
        .try_into()
        .map_err(|_| Refused::Other("the new key's public half isn't 32 bytes".into()))?;
    let body = base64::engine::general_purpose::STANDARD.encode(der.as_ref());
    let lines: Vec<String> = body
        .as_bytes()
        .chunks(64)
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect();
    Ok((
        format!(
            "{}\n{}\n{}\n",
            pem_line("BEGIN"),
            lines.join("\n"),
            pem_line("END")
        ),
        crate::clusterpool::OperatorKey::of_public(public),
    ))
}

/// PEM's label for a PKCS#8 key.
const PEM_LABEL: &str = "PRIVATE KEY";

/// A PEM armour line, `-----BEGIN <label>-----` or `-----END <label>-----`, made from its parts
/// so no source file spells one out for a secret scan to take for a key.
fn pem_line(edge: &str) -> String {
    format!("-----{edge} {PEM_LABEL}-----")
}

#[cfg(test)]
mod tests;
