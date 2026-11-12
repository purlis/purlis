// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Pool setup on a cluster** (#4492, #4493): what Where, Check the cluster and Install find, read
//! through the person's own `kubectl`, on the context they chose, with their own credentials.
//!
//! - **Nothing of a credential is kept**: the choices hold a context's name, and every read asks
//!   kubectl, which reads the person's kubeconfig as it is now ([`Kubectl::contexts`] reads
//!   `kubectl config view`, which leaves every credential out).
//! - **Doctor's checks are the operator's own** ([`purlis_cluster_doctor`]), asked here through
//!   kubectl ([`Asked`]), so Check the cluster and `purlis-cluster-operator doctor` say the same
//!   of the same cluster. A cluster that lacks something the install doesn't apply stops there.
//! - **An operator already installed is passed through**: Check the cluster and Install read
//!   done, with what was found, and doctor isn't asked (a teammate may not hold its rights).
//! - **The signing key is only looked for**, never read: its Secret is asked after through
//!   kubectl's table, which the API server fills with its name and how many keys it holds, and
//!   never with what they are.

use purlis_cluster_doctor::{self as doctor, Cluster, Controller, Unanswered, Version};

use crate::clusterpool::Kubectl;

/// The namespace the operator is installed in, as the install makes it.
pub const SYSTEM_NAMESPACE: &str = "purlis-system";

/// The operator's deployment there.
pub const OPERATOR_DEPLOYMENT: &str = "purlis-cluster-operator";

/// The Secret holding the key the operator signs runners' link keys with (#2239).
pub const SIGNING_KEY_SECRET: &str = "purlis-cluster-operator-signing-key";

/// purlis's own definitions, which the install's definitions step applies.
pub const DEFINITIONS: [&str; 2] = ["pools.runners.purlis.dev", "claims.runners.purlis.dev"];

/// How long one read of a cluster's setup may take, all its kubectl runs together.
pub const READ_WITHIN: std::time::Duration = std::time::Duration::from_secs(30);

/// How the operator's deployment stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operator {
    Absent,
    /// It is there, and no replica is ready yet.
    Starting,
    /// A replica is ready, running `image`.
    Running {
        image: String,
    },
}

/// **What the cluster holds of the install**, part by part (#4493).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub definitions: bool,
    pub key: bool,
    pub operator: Operator,
    /// How many Claims there are, and how many people made them, where they could be counted:
    /// what an uninstall would end.
    pub claims: Option<(usize, usize)>,
}

/// A cluster as Pool setup reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClusterFacts {
    /// Why there is no kubectl to ask, where there is none.
    pub no_kubectl: Option<String>,
    /// The kubeconfig's contexts, and the one it is on.
    pub contexts: Vec<String>,
    pub current: Option<String>,
    /// Why the contexts couldn't be read, where they couldn't.
    pub unread: Option<String>,
    /// The context chosen on Where, while the kubeconfig still has it.
    pub chosen: Option<String>,
    /// What doctor found on the chosen context, where it was asked.
    pub doctor: Option<doctor::Report>,
    /// What the chosen context's cluster holds of the install, where it was read.
    pub installed: Option<Result<Installed, String>>,
    /// The cluster's Pools, by name, where its operator runs and they could be listed (#4494).
    pub pools: Option<Vec<String>>,
    /// **Whether the person joins their team's install** (#4495): purlis's definitions are served
    /// on the chosen context's cluster, and their sign-in may not read the install itself. Then
    /// nothing of the install is read or offered, and doctor isn't asked: those are an admin's.
    pub joining: bool,
}

/// **What this machine keeps of a cluster pool** (#4494): where it reaches its cluster, and the
/// person's own mark. Nothing of a credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptHere {
    pub context: String,
    /// The cluster's Pool it claims its runner of.
    pub pool: String,
    /// The person's mark that it is their own (#4071).
    pub owned: bool,
    /// Whether the cluster's Pool mints the agent's GitHub App tokens there (#4204).
    pub mints: bool,
    /// The cluster operator key pinned here, spelled `ed25519:<64 hex>`: no secret, and what a
    /// teammate checks theirs against as they join (#4495).
    pub pinned: String,
    /// How that pin stands against what the Pool names now (#4495).
    pub now: crate::pools::PinNow,
}

impl ClusterFacts {
    /// The context purlis recommends: the kubeconfig's current one, else its only one.
    #[must_use]
    pub fn recommended(&self) -> Option<&str> {
        self.current
            .as_deref()
            .filter(|current| self.contexts.iter().any(|one| one == current))
            .or(match self.contexts.as_slice() {
                [only] => Some(only.as_str()),
                _ => None,
            })
    }

    /// Whether the operator is installed and running on the chosen context's cluster.
    #[must_use]
    pub fn operator_runs(&self) -> bool {
        matches!(
            &self.installed,
            Some(Ok(Installed {
                operator: Operator::Running { .. },
                ..
            }))
        )
    }

    /// Whether the pinned upstream release is installed, as doctor found it.
    #[must_use]
    pub fn upstream_installed(&self) -> bool {
        self.operator_runs()
            || self
                .doctor
                .as_ref()
                .is_some_and(|report| report.findings.iter().all(|finding| !finding.install))
    }
}

/// **Doctor's questions, asked through kubectl** with the person's own credentials. Every one
/// only reads; `auth can-i` is the question the operator's own doctor asks
/// (`SelfSubjectAccessReview`), which the API server keeps nothing of. A read that fails is
/// asked about again with `auth can-i`, so a refusal is told from a failure by the cluster's
/// answer and not by kubectl's words.
pub struct Asked<'a> {
    pub kubectl: &'a Kubectl,
    pub context: &'a str,
    /// The API server the context names, where kubectl said, as doctor names the cluster it asked.
    pub server: Option<String>,
}

impl Asked<'_> {
    fn failed(
        &self,
        verb: &str,
        resource: &str,
        namespace: Option<&str>,
        said: String,
    ) -> Unanswered {
        let mut args = vec!["auth", "can-i", verb, resource];
        if let Some(namespace) = namespace {
            args.extend(["--namespace", namespace]);
        }
        match self.kubectl.asked(&args, None) {
            Ok(answer) if !answer.ok && answer.printed.trim() == "no" => Unanswered::Forbidden,
            _ => Unanswered::Failed(crate::shown::short(&said)),
        }
    }

    /// One object, or `None` where there is none (`--ignore-not-found` prints nothing).
    fn object(
        &self,
        resource: &str,
        name: &str,
        namespace: Option<&str>,
    ) -> Result<Option<serde_json::Value>, Unanswered> {
        let mut args = vec![
            "get",
            resource,
            name,
            "--ignore-not-found",
            "--output",
            "json",
        ];
        if let Some(namespace) = namespace {
            args.extend(["--namespace", namespace]);
        }
        let answer = self
            .kubectl
            .asked(&args, None)
            .map_err(|why| Unanswered::Failed(crate::shown::short(&why.to_string())))?;
        if !answer.ok {
            return Err(self.failed("get", resource, namespace, answer.said));
        }
        if answer.printed.trim().is_empty() {
            return Ok(None);
        }
        serde_json::from_str(&answer.printed)
            .map(Some)
            .map_err(|_| Unanswered::Failed(format!("kubectl get {resource} did not answer JSON")))
    }
}

impl Cluster for Asked<'_> {
    fn asked(&self) -> String {
        match &self.server {
            Some(server) => format!("context {}, API server {server}", self.context),
            None => format!("context {}", self.context),
        }
    }

    async fn version(&self) -> Result<Version, Unanswered> {
        let answer = self
            .kubectl
            .asked(&["version", "--output", "json"], None)
            .map_err(|why| Unanswered::Failed(crate::shown::short(&why.to_string())))?;
        let json: Option<serde_json::Value> = serde_json::from_str(&answer.printed).ok();
        let server = json.as_ref().and_then(|json| json.get("serverVersion"));
        let Some(server) = server else {
            return Err(Unanswered::Failed(crate::shown::short(
                if answer.said.is_empty() {
                    "kubectl version named no server version"
                } else {
                    &answer.said
                },
            )));
        };
        let field = |name: &str| {
            server
                .get(name)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        Ok(Version {
            major: field("major"),
            minor: field("minor"),
            git_version: field("gitVersion"),
        })
    }

    async fn definition(&self, name: &str) -> Result<Option<Vec<String>>, Unanswered> {
        let found = self.object("customresourcedefinitions.apiextensions.k8s.io", name, None)?;
        Ok(found.map(|definition| {
            definition
                .pointer("/spec/versions")
                .and_then(serde_json::Value::as_array)
                .map(|versions| {
                    versions
                        .iter()
                        .filter(|version| {
                            version.get("served") == Some(&serde_json::Value::Bool(true))
                        })
                        .filter_map(|version| {
                            version.get("name").and_then(serde_json::Value::as_str)
                        })
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        }))
    }

    async fn controller(&self) -> Result<Option<Controller>, Unanswered> {
        let (namespace, name) = doctor::AGENT_SANDBOX_CONTROLLER;
        let found = self.object("deployments.apps", name, Some(namespace))?;
        Ok(found.map(|deployment| Controller {
            ready: ready_of(&deployment),
            images: images_of(&deployment),
        }))
    }

    async fn may(&self, verb: &str, group: &str, resource: &str) -> Result<bool, Unanswered> {
        let what = if group.is_empty() {
            resource.to_owned()
        } else {
            format!("{resource}.{group}")
        };
        let answer = self
            .kubectl
            .asked(&["auth", "can-i", verb, &what], None)
            .map_err(|why| Unanswered::Failed(crate::shown::short(&why.to_string())))?;
        match (answer.ok, answer.printed.trim()) {
            (true, "yes") => Ok(true),
            (false, "no") => Ok(false),
            _ => Err(Unanswered::Failed(crate::shown::short(&answer.said))),
        }
    }

    async fn marked_namespaces(&self) -> Result<Vec<String>, Unanswered> {
        let (key, value) = doctor::MANAGED_BY;
        let label = format!("{key}={value}");
        let answer = self
            .kubectl
            .asked(
                &[
                    "get",
                    "namespaces",
                    "--selector",
                    &label,
                    "--output",
                    "json",
                ],
                None,
            )
            .map_err(|why| Unanswered::Failed(crate::shown::short(&why.to_string())))?;
        if !answer.ok {
            return Err(self.failed("list", "namespaces", None, answer.said));
        }
        let listed: serde_json::Value = serde_json::from_str(&answer.printed)
            .map_err(|_| Unanswered::Failed("kubectl get namespaces did not answer JSON".into()))?;
        Ok(listed
            .get("items")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        item.pointer("/metadata/name")
                            .and_then(serde_json::Value::as_str)
                    })
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default())
    }
}

fn ready_of(deployment: &serde_json::Value) -> i32 {
    deployment
        .pointer("/status/readyReplicas")
        .and_then(serde_json::Value::as_i64)
        .and_then(|ready| i32::try_from(ready).ok())
        .unwrap_or(0)
}

fn images_of(deployment: &serde_json::Value) -> Vec<String> {
    deployment
        .pointer("/spec/template/spec/containers")
        .and_then(serde_json::Value::as_array)
        .map(|containers| {
            containers
                .iter()
                .filter_map(|container| container.get("image").and_then(serde_json::Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// **Doctor, on the context `asked` names.** It only reads.
#[must_use]
pub fn doctor(asked: &Asked<'_>) -> doctor::Report {
    futures::executor::block_on(doctor::check(asked))
}

/// **What the cluster holds of the install**, read through `kubectl`. Only reads, and the
/// signing key's Secret is only looked for: kubectl's table names it, and never what it holds.
pub fn installed(kubectl: &Kubectl) -> Result<Installed, String> {
    let asked = |args: &[&str]| {
        kubectl
            .asked(args, None)
            .map_err(|why| why.to_string())
            .and_then(|answer| {
                if answer.ok {
                    Ok(answer.printed)
                } else {
                    Err(crate::shown::short(&answer.said))
                }
            })
    };
    let mut definitions = true;
    for name in DEFINITIONS {
        let found = asked(&[
            "get",
            "customresourcedefinitions.apiextensions.k8s.io",
            name,
            "--ignore-not-found",
            "--output",
            "name",
        ])?;
        definitions &= !found.trim().is_empty();
    }
    // The table of the Secret, never the Secret: the API server fills its columns (its name, type
    // and how many keys it holds), so what it holds never leaves the cluster.
    let key = !asked(&[
        "get",
        "secrets",
        SIGNING_KEY_SECRET,
        "--namespace",
        SYSTEM_NAMESPACE,
        "--ignore-not-found",
        "--no-headers",
    ])?
    .trim()
    .is_empty();
    let deployment = asked(&[
        "get",
        "deployments.apps",
        OPERATOR_DEPLOYMENT,
        "--namespace",
        SYSTEM_NAMESPACE,
        "--ignore-not-found",
        "--output",
        "json",
    ])?;
    let operator = if deployment.trim().is_empty() {
        Operator::Absent
    } else {
        let deployment: serde_json::Value = serde_json::from_str(&deployment)
            .map_err(|_| "kubectl get deployments did not answer JSON".to_owned())?;
        match (
            ready_of(&deployment),
            images_of(&deployment).into_iter().next(),
        ) {
            (ready, Some(image)) if ready > 0 => Operator::Running { image },
            _ => Operator::Starting,
        }
    };
    let claims = if definitions {
        asked(&[
            "get",
            "claims.runners.purlis.dev",
            "--all-namespaces",
            "--output",
            "json",
        ])
        .ok()
        .and_then(|listed| serde_json::from_str::<serde_json::Value>(&listed).ok())
        .and_then(|listed| {
            listed
                .get("items")
                .and_then(serde_json::Value::as_array)
                .cloned()
        })
        .map(|items| {
            let people: std::collections::BTreeSet<&str> = items
                .iter()
                .filter_map(|item| {
                    item.pointer("/spec/requester")
                        .and_then(serde_json::Value::as_str)
                })
                .collect();
            (items.len(), people.len())
        })
    } else {
        Some((0, 0))
    };
    Ok(Installed {
        definitions,
        key,
        operator,
        claims,
    })
}

/// The cluster's Pools, by name; `None` where they couldn't be listed.
fn pools_of(kubectl: &Kubectl) -> Option<Vec<String>> {
    let answer = kubectl
        .asked(
            &["get", "pools.runners.purlis.dev", "--output", "json"],
            None,
        )
        .ok()
        .filter(|answer| answer.ok)?;
    let listed: serde_json::Value = serde_json::from_str(&answer.printed).ok()?;
    Some(
        listed
            .get("items")?
            .as_array()?
            .iter()
            .filter_map(|item| {
                item.pointer("/metadata/name")
                    .and_then(serde_json::Value::as_str)
            })
            .filter(|name| crate::clusterpool::is_subdomain(name))
            .map(str::to_owned)
            .collect(),
    )
}

/// **Reads a cluster for Pool setup**: the kubeconfig's contexts, and on the chosen one, what the
/// cluster holds of the install and, before an operator runs there, doctor. `deep` is whether the
/// cluster itself is asked: only where the setup is on a cluster.
#[must_use]
pub fn read(kubectl: Result<&Kubectl, String>, chosen: Option<&str>, deep: bool) -> ClusterFacts {
    let kubectl = match kubectl {
        Ok(kubectl) => kubectl,
        Err(why) => {
            return ClusterFacts {
                no_kubectl: Some(why),
                ..ClusterFacts::default()
            };
        }
    };
    let mut facts = match kubectl.contexts() {
        Ok((contexts, current)) => ClusterFacts {
            contexts,
            current,
            ..ClusterFacts::default()
        },
        Err(why) => ClusterFacts {
            unread: Some(crate::shown::short(&why.to_string())),
            ..ClusterFacts::default()
        },
    };
    facts.chosen = chosen
        .filter(|chosen| facts.contexts.iter().any(|one| one == chosen))
        .map(str::to_owned);
    let Some(context) = facts.chosen.clone().filter(|_| deep) else {
        return facts;
    };
    let on = kubectl
        .clone()
        .with_context(&context)
        .all_within(READ_WITHIN);
    let read = installed(&on);
    if read.is_err() && joins(&on) {
        facts.joining = true;
        facts.pools = pools_of(&on);
        return facts;
    }
    facts.installed = Some(read);
    if facts.operator_runs() {
        facts.pools = pools_of(&on);
    }
    if !facts.operator_runs() {
        facts.doctor = Some(doctor(&Asked {
            kubectl: &on,
            context: &context,
            server: server_of(&on),
        }));
    }
    facts
}

/// **Whether the person behind `kubectl` joins their team's install** (#4495), asked only once the
/// install couldn't be read: purlis's definitions are served there, as discovery (which every
/// signed-in person may ask) says, and the cluster answers no to whether they may read the
/// operator's deployment. A read that failed for any other reason is no join: it stays a failure
/// to read, with its own words.
fn joins(kubectl: &Kubectl) -> bool {
    let served = kubectl
        .asked(
            &[
                "api-resources",
                "--api-group",
                "runners.purlis.dev",
                "--output",
                "name",
            ],
            None,
        )
        .ok()
        .filter(|answer| answer.ok)
        .is_some_and(|answer| {
            answer
                .printed
                .lines()
                .any(|line| line.trim() == DEFINITIONS[0])
        });
    if !served {
        return false;
    }
    matches!(
        kubectl.asked(
            &[
                "auth",
                "can-i",
                "get",
                "deployments.apps",
                "--namespace",
                SYSTEM_NAMESPACE,
            ],
            None,
        ),
        Ok(answer) if !answer.ok && answer.printed.trim() == "no"
    )
}

/// The API server the context of `kubectl` names, as kubectl's own view of it says.
fn server_of(kubectl: &Kubectl) -> Option<String> {
    let answer = kubectl
        .asked(&["config", "view", "--minify", "--output", "json"], None)
        .ok()
        .filter(|answer| answer.ok)?;
    let view: serde_json::Value = serde_json::from_str(&answer.printed).ok()?;
    view.pointer("/clusters/0/cluster/server")
        .and_then(serde_json::Value::as_str)
        .map(crate::shown::short)
}

/// **This machine's kubectl**, found as a cluster pool's is, or why there is none.
pub fn kubectl_here() -> Result<Kubectl, String> {
    let path = crate::envvar::var("PATH");
    let home = crate::envvar::var_os("HOME").map(std::path::PathBuf::from);
    Kubectl::on_this_machine(path.as_deref(), home.as_deref()).map_err(|why| why.to_string())
}

use super::{Action, ActionId, Choices, Facts, Kind, Reading, action};

/// kubectl's own install page.
pub const KUBECTL_INSTALL_PAGE: &str = "https://kubernetes.io/docs/tasks/tools/";

/// **Where, for a cluster** (#4492): a context to reach it by, recommended from the kubeconfig,
/// and kept only once the person chooses it. Docker is the other choice ([`also_docker`]).
pub(super) fn where_reading(facts: &Facts, choices: &Choices) -> Reading {
    let cluster = &facts.cluster;
    if let Some(why) = &cluster.no_kubectl {
        return Reading::attention(
            format!(
                "A cluster is reached through kubectl, with your own sign-in, and {why}. You can \
                 use Docker on this machine instead."
            ),
            Some(Action {
                id: ActionId::GetKubectl,
                label: "Open kubectl's install page".to_owned(),
                does: "Opens Kubernetes' own page in your browser. purlis doesn't install \
                       anything, and checks again by itself."
                    .to_owned(),
                command: None,
                url: Some(KUBECTL_INSTALL_PAGE.to_owned()),
            }),
        );
    }
    if let Some(why) = &cluster.unread {
        return Reading::attention(
            format!("purlis couldn't read your kubeconfig's contexts: {why}"),
            action(
                ActionId::CheckAgain,
                "Check again",
                "Reads your kubeconfig again. It changes nothing.",
            ),
        );
    }
    if cluster.contexts.is_empty() {
        return Reading::attention(
            "Your kubeconfig names no cluster, so there is none to set up here. Ask your team for \
             access to its cluster, or use Docker on this machine.",
            action(
                ActionId::UseDocker,
                "Use Docker instead",
                "Saves your choice. Nothing on this machine changes yet.",
            ),
        );
    }
    if let Some(kept) = facts.pool.as_ref().and_then(|pool| pool.cluster.as_ref()) {
        return Reading::done(format!(
            "Your team's cluster, through context {}",
            kept.context
        ));
    }
    if choices.kind == Some(Kind::Cluster)
        && let Some(context) = &cluster.chosen
    {
        return Reading::done(format!("Your team's cluster, through context {context}"));
    }
    let recommended = cluster.recommended();
    let found = match recommended {
        Some(context) => format!(
            "Your kubeconfig reaches a cluster through context {context}, so purlis recommends \
             running chats there. purlis uses your own sign-in and keeps none of it."
        ),
        None => {
            "Your kubeconfig names more than one cluster. Choose the one to use under Options. \
                 purlis uses your own sign-in and keeps none of it."
                .to_owned()
        }
    };
    Reading::to_do(
        found,
        action(
            ActionId::UseCluster,
            "Continue with this cluster",
            "Saves your choice of context. Nothing on the cluster changes yet.",
        ),
    )
}

/// The other choice on Where, where there is one: Docker for a setup on a cluster, a cluster for
/// a setup on Docker where the kubeconfig names one.
pub(super) fn also(kind: Kind, facts: &Facts) -> Option<Action> {
    match kind {
        Kind::Cluster => action(
            ActionId::UseDocker,
            "Use Docker on this machine instead",
            "Saves your choice. Nothing on this machine changes yet.",
        ),
        Kind::Docker if !facts.cluster.contexts.is_empty() => action(
            ActionId::UseCluster,
            "Use your team's cluster instead",
            "Saves your choice of context. Nothing on the cluster changes yet.",
        ),
        Kind::Docker => None,
    }
}

fn check_again(does: &str) -> Option<Action> {
    action(ActionId::CheckAgain, "Check again", does)
}

/// **Check the cluster**: doctor's findings, first (#4492, #2239). What the install's own steps
/// apply is left to Install; anything else stops the set-up here, with what it lacks.
pub(super) fn check_reading(facts: &Facts) -> Reading {
    let cluster = &facts.cluster;
    if let Some(Ok(Installed {
        operator: Operator::Running { image },
        ..
    })) = &cluster.installed
    {
        let mut reading = Reading::done("purlis's cluster operator runs here");
        reading.findings = vec![format!(
            "purlis's cluster operator is installed and running here, from {}, so doctor, which \
             checks a cluster before the install, isn't asked.",
            crate::shown::short(image)
        )];
        return reading;
    }
    if cluster.joining {
        let mut reading = Reading::done("Installed by your team");
        reading.findings = vec![
            "purlis is on this cluster already, installed by your team, so there's nothing to \
             check or install. Your sign-in doesn't need to read the install itself."
                .to_owned(),
        ];
        return reading;
    }
    let Some(report) = &cluster.doctor else {
        return Reading::to_do(
            "Doctor checks what the cluster lacks for purlis, before anything changes.",
            check_again("Asks the cluster again. It changes nothing."),
        );
    };
    if report.lacks_more_than_the_install() {
        return Reading::attention(
            "The cluster lacks something the install doesn't add, so nothing changes until it \
             has it. Each line says what to do, and this step checks again.",
            check_again("Asks the cluster again. It changes nothing."),
        );
    }
    let mut reading = Reading::done(if report.lacks_anything() {
        "Ready for the install"
    } else {
        "Nothing missing"
    });
    reading.findings = vec![if report.lacks_anything() {
        "The cluster has what purlis needs. What it lacks, the Install step adds.".to_owned()
    } else {
        "The cluster has what purlis needs.".to_owned()
    }];
    reading
}

/// **Doctor's lines, as Check the cluster shows them** (#4492): what holds, and what the cluster
/// lacks with its next step, in doctor's own words. What the install adds says so instead of
/// naming a checkout's folder.
#[must_use]
pub fn checks_of(facts: &Facts) -> Vec<super::Check> {
    use super::{Check, CheckOf, Verdict};
    let cluster = &facts.cluster;
    if let Some(Ok(Installed {
        operator: Operator::Running { image },
        ..
    })) = &cluster.installed
    {
        return vec![Check {
            about: CheckOf::Cluster,
            verdict: Verdict::Ok,
            title: "purlis's cluster operator runs".to_owned(),
            more: Some(crate::shown::short(image)),
        }];
    }
    let Some(report) = &cluster.doctor else {
        return Vec::new();
    };
    report
        .holds
        .iter()
        .map(|line| Check {
            about: CheckOf::Cluster,
            verdict: Verdict::Ok,
            title: line.clone(),
            more: None,
        })
        .chain(report.findings.iter().map(|finding| Check {
            about: CheckOf::Cluster,
            verdict: if finding.install {
                Verdict::Todo
            } else {
                Verdict::Missing
            },
            title: finding.lacks.clone(),
            more: Some(if finding.install {
                "The Install step adds it.".to_owned()
            } else {
                format!("Next: {}.", finding.next)
            }),
        }))
        .collect()
}

/// **Install** (#4493): done once the cluster holds every part of it and the operator runs; else
/// the next part's own button.
pub(super) fn install_reading(facts: &Facts, choices: &Choices) -> Reading {
    let cluster = &facts.cluster;
    if cluster.joining {
        let mut reading = Reading::done("Installed by your team");
        reading.findings = vec![
            "purlis's cluster operator is on this cluster, installed by your team. Installing, \
             changing and uninstalling it are your cluster admin's, so there's nothing to do here."
                .to_owned(),
        ];
        return reading;
    }
    match &cluster.installed {
        Some(Ok(_)) => {
            let parts = parts_of(facts, choices);
            if parts.iter().all(|part| part.done) {
                return Reading::done("Installed");
            }
            Reading::to_do(
                "Install puts purlis's cluster operator on the cluster with your own sign-in, a \
                 part at a time. Each button says what it applies, and nothing changes until you \
                 press it.",
                parts.iter().find_map(|part| part.next.clone()),
            )
        }
        Some(Err(why)) => Reading::attention(
            format!("purlis couldn't read what the cluster holds of the install: {why}"),
            check_again("Asks the cluster again. It changes nothing."),
        ),
        None => Reading::to_do("", None),
    }
}

/// The words that point a command by hand at the context chosen (` --context <name>`).
fn on_of(facts: &Facts) -> String {
    facts
        .cluster
        .chosen
        .as_deref()
        .or(facts.cluster.recommended())
        .map(crate::shellseg::shell_quote)
        .map_or_else(String::new, |context| format!(" --context {context}"))
}

/// **Install's parts as the cluster holds them** (#4493).
pub(super) fn parts_of(facts: &Facts, choices: &Choices) -> Vec<super::install::Part> {
    match &facts.cluster.installed {
        Some(Ok(installed)) => super::install::parts(
            facts.cluster.upstream_installed(),
            installed,
            choices.operator_image.as_deref(),
            &on_of(facts),
        ),
        _ => Vec::new(),
    }
}

/// **How the install is undone, never by a button** (story 56): where something of it is there.
pub(super) fn undo_of(facts: &Facts) -> Option<super::See> {
    match &facts.cluster.installed {
        Some(Ok(installed))
            if installed.definitions
                || installed.key
                || !matches!(installed.operator, Operator::Absent) =>
        {
            Some(super::install::uninstall(installed, &on_of(facts)))
        }
        _ => None,
    }
}

/// **What a cluster's step does by hand** (Q30): the commands behind its See what this does.
pub(super) fn see_of(id: super::StepId, facts: &Facts, choices: &Choices) -> Option<super::See> {
    use super::{See, StepId};
    let on = on_of(facts);
    // A teammate is shown no admin's commands: doctor's, the install's, making a Pool or the
    // minting opt-in (#4495).
    if facts.cluster.joining && id != StepId::Where {
        return None;
    }
    match id {
        StepId::Where => Some(See {
            note: "This is how purlis lists your kubeconfig's contexts. It only reads, and \
                   leaves out what each signs in with."
                .to_owned(),
            text: "kubectl config get-contexts".to_owned(),
        }),
        StepId::Check => Some(See {
            note: "Doctor asks these, with your own sign-in. Each only reads; auth can-i asks the \
                   cluster what you may do, and it keeps nothing of the question. \
                   purlis-cluster-operator doctor asks the same, from a checkout of purlis."
                .to_owned(),
            text: doctor_by_hand(&on),
        }),
        StepId::Pool if facts.pool.is_none() => Some(See {
            note:
                "Creating the pool applies this, with your own sign-in. Adding it here then pins \
                   the operator's key and claims your runner, which only the window does: \
                   there's no command for it."
                    .to_owned(),
            text: format!(
                "kubectl{on} apply --server-side -f - <<'POOL'\n{}POOL",
                pool_definition("<name>", "ghcr.io/purlis/purlis-runner@sha256:<digest>")
            ),
        }),
        StepId::Identity => facts
            .pool
            .as_ref()
            .and_then(|pool| pool.cluster.as_ref())
            .filter(|kept| !kept.mints)
            .map(|kept| See {
                note: "Your cluster can mint the agent's tokens itself, so its chats go on while \
                       this machine is away. Recommended once Try it has proved the identity. \
                       Your cluster's admin runs these."
                    .to_owned(),
                text: minting_by_hand(&on, &kept.pool),
            }),
        StepId::Install => Some(See {
            note: "These are the install's commands, a part after another, as each button runs \
                   them, from a checkout of purlis at this version. Run them yourself if you \
                   prefer, and purlis marks each part done."
                .to_owned(),
            text: super::install::all_by_hand(choices.operator_image.as_deref(), &on),
        }),
        _ => None,
    }
}

/// The questions doctor asks, as kubectl commands.
pub(super) fn doctor_by_hand(on: &str) -> String {
    let (namespace, controller) = doctor::AGENT_SANDBOX_CONTROLLER;
    let mut lines = vec![format!("kubectl{on} version")];
    for (name, _) in doctor::AGENT_SANDBOX_DEFINITIONS {
        lines.push(format!("kubectl{on} get customresourcedefinitions {name}"));
    }
    lines.push(format!(
        "kubectl{on} --namespace {namespace} get deployment {controller}"
    ));
    for (group, resource) in doctor::CLUSTER_SCOPED {
        let what = if group.is_empty() {
            (*resource).to_owned()
        } else {
            format!("{resource}.{group}")
        };
        lines.push(format!("kubectl{on} auth can-i create {what}"));
        lines.push(format!("kubectl{on} auth can-i patch {what}"));
    }
    for (verb, group, resource) in doctor::ROLE_RIGHTS {
        lines.push(format!("kubectl{on} auth can-i {verb} {resource}.{group}"));
    }
    let (key, value) = doctor::MANAGED_BY;
    lines.push(format!(
        "kubectl{on} get namespaces --selector {key}={value}"
    ));
    lines.join("\n")
}

/// **The cluster pool `name` this machine keeps, read as Pool setup reads a pool** (#4494):
/// `None` where there is no cluster pool of that name. Its runner is what this machine keeps of
/// its claim, and nothing is asked of the cluster for it; its sign-ins are asked of the runner
/// only where the person marked the pool as their own, through the gate a profile passes there.
pub(super) fn pool_kept(
    here: &super::Here<'_>,
    name: &str,
) -> Result<Option<super::PoolFacts>, crate::runners::Refused> {
    use super::{PoolFacts, Profile, Runner, SignIns};
    use crate::runners;
    let Some(pool) = runners::cluster_pools(here.config)?
        .into_iter()
        .find(|pool| pool.name() == name)
    else {
        return Ok(None);
    };
    let kept = runners::cluster_runner_for(here.config, name)?;
    let runner = if kept.is_some() {
        Runner::Running
    } else {
        Runner::None
    };
    let sign_ins = match (&kept, here.kubectl) {
        _ if !pool.owned() => SignIns::NotYours,
        _ if crate::runnertaint::pool_ran_unsandboxed(here.config, name) => SignIns::Tainted,
        (None, _) | (_, Err(_)) => SignIns::NotAsked,
        (Some((cluster, kept)), Ok(kubectl)) => {
            let default = here
                .plane
                .and_then(|root| crate::profiles::current(root).default);
            match crate::pools::cluster_runner_yours(
                here.config,
                name,
                cluster,
                kept,
                &crate::pools::vouched_now,
            ) {
                Err(why) => SignIns::CannotTell(crate::shown::short(&why)),
                Ok(judged) => {
                    let mut read = Vec::new();
                    let mut unread = None;
                    for profile in pool.profiles_kept(default.as_deref()) {
                        match crate::pools::cluster_standing(
                            here.config,
                            kubectl,
                            name,
                            &judged,
                            &profile,
                        ) {
                            Ok(standing) => read.push((
                                profile,
                                standing.map_or(Profile::NotKept, Profile::Standing),
                            )),
                            Err(why) => {
                                unread = Some(crate::shown::short(&why));
                                break;
                            }
                        }
                    }
                    unread.map_or(SignIns::Read(read), SignIns::CannotTell)
                }
            }
        }
    };
    Ok(Some(PoolFacts {
        name: name.to_owned(),
        runner,
        sign_ins,
        cluster: Some(KeptHere {
            context: pool.context().to_owned(),
            pool: pool.pool().to_owned(),
            owned: pool.owned(),
            mints: pool.mints_in_cluster(),
            pinned: pool.operator().spelled(),
            now: match here.kubectl {
                Ok(kubectl) => crate::pools::pin_now(
                    here.config,
                    &kubectl.clone().all_within(READ_WITHIN),
                    name,
                    runners::now(),
                ),
                Err(why) => crate::pools::PinNow::Unread(why.to_owned()),
            },
        }),
    }))
}

/// **The Pool step on a cluster** (#4494): with no pool here yet, a Pool of the cluster's to
/// claim a runner of, or one to make there first; with one, what it is.
pub(super) fn pool_reading(facts: &Facts) -> Reading {
    if let Some(pool) = &facts.pool {
        if let Some(kept) = &pool.cluster {
            match &kept.now {
                crate::pools::PinNow::Changed { .. } => {
                    return Reading::attention(
                        format!(
                            "Your cluster's pool {} now names another cluster operator key than \
                             the one pinned on this machine, and the pinned key didn't vouch for \
                             it. purlis refuses every link to your runner until you check the \
                             new key with your cluster's admin and pin it.",
                            kept.pool
                        ),
                        action(
                            ActionId::CheckKeyAgain,
                            "Check the new key",
                            "Shows the key the pool names now beside the one pinned. Nothing is \
                             pinned until you paste the key your admin gives you.",
                        ),
                    );
                }
                crate::pools::PinNow::OtherCluster => {
                    return Reading::attention(
                        format!(
                            "Your cluster's pool {} now says it is in another cluster than the one \
                             pinned on this machine, so purlis refuses every link to your runner \
                             and pins nothing of it. If your team moved it, remove the pool here \
                             and add it again.",
                            kept.pool
                        ),
                        action(
                            ActionId::RemovePool,
                            "Open Pools",
                            "Opens Settings › Pools, where the pool is removed. Nothing changes \
                             until you press Remove there.",
                        ),
                    );
                }
                crate::pools::PinNow::Holds | crate::pools::PinNow::Unread(_) => {}
            }
        }
        return super::pool_reading(facts);
    }
    if facts.cluster.joining
        && facts
            .cluster
            .pools
            .as_ref()
            .is_none_or(|pools| pools.is_empty())
    {
        return Reading::to_do(
            "A pool is a named place your chats run. Your sign-in can't list your team's pools, \
             so type the pool's name your cluster's admin gave you. purlis reads the key it \
             names, for you to check against the one your admin gave you too.",
            action(
                ActionId::AddClusterPool,
                "Read the pool's key",
                "Reads the cluster operator key the pool names. Nothing is pinned or claimed yet.",
            ),
        );
    }
    match &facts.cluster.pools {
        None => Reading::to_do(
            "A pool is a named place your chats run. On a cluster, each chat's runner is a pod the \
             cluster operator makes from it.",
            None,
        ),
        Some(pools) if pools.is_empty() => Reading::to_do(
            "A pool is a named place your chats run. Your cluster has none yet, so this makes \
             one, from purlis's runner image, and then claims your runner of it.",
            action(
                ActionId::CreateClusterPool,
                "Create the pool",
                "Applies a Pool to the cluster with your own sign-in. Anyone your cluster lets \
                 use it can claim a runner of it.",
            ),
        ),
        Some(pools) => Reading::to_do(
            format!(
                "Your team has {} on this cluster. Adding one here claims a runner of it for you \
                 and pairs it with this machine.",
                match pools.as_slice() {
                    [one] => format!("the pool {one}"),
                    more => format!("these pools: {}", more.join(", ")),
                }
            ),
            action(
                ActionId::AddClusterPool,
                "Add the pool here",
                "Pins the cluster operator key the pool names, then claims your runner of it and \
                 pairs it with this machine. Nothing else on the cluster changes.",
            ),
        ),
    }
}

/// **The Pool a set-up makes in a cluster** (#4494): named `name`, its runners of `image`, by
/// digest. Every other field is the operator's default.
#[must_use]
pub fn pool_definition(name: &str, image: &str) -> String {
    format!(
        "apiVersion: runners.purlis.dev/v1alpha1\n\
         kind: Pool\n\
         metadata:\n  name: {name}\n\
         spec:\n  image: {image}\n"
    )
}

/// **Makes the Pool `name` in the cluster `kubectl` is pointed at**, of runner image `image` by
/// digest, with the person's own sign-in. Refused for a name or an image the cluster would refuse.
pub fn create_pool(
    kubectl: &Kubectl,
    name: &str,
    image: &str,
) -> Result<(), crate::runners::Refused> {
    use crate::runners::Refused;
    if !crate::clusterpool::is_subdomain(name) {
        return Err(Refused::Other(format!(
            "nothing was made: \"{}\" isn't a pool's name; use lower-case letters, digits, \
             dashes and dots",
            crate::shown::short(name)
        )));
    }
    if !super::install::is_by_digest(image) {
        return Err(Refused::Other(
            "nothing was made: name the runner image by its digest, <repository>@sha256:…".into(),
        ));
    }
    let answer = kubectl
        .asked(
            &["apply", "--server-side", "-f", "-"],
            Some(&pool_definition(name, image)),
        )
        .map_err(|why| Refused::Other(why.to_string()))?;
    if !answer.ok {
        return Err(Refused::Other(format!(
            "the cluster didn't take the pool: {}",
            crate::shown::short(&answer.said)
        )));
    }
    Ok(())
}

/// **What making the agent's tokens in the cluster takes, by hand** (#4494, B on #4469): the
/// App's own key, made on GitHub by the admin for this cluster, never a vault's.
fn minting_by_hand(on: &str, pool: &str) -> String {
    format!(
        "# Opt-in: the cluster mints the agent's GitHub App tokens, so chats go on while this\n\
         # machine is away. Make a key for the App on GitHub for this cluster alone; no vault's\n\
         # key ever moves here. docs/cluster-operator.md, \"Minting GitHub App tokens in the\n\
         # cluster\", has the Role a plain install adds for it.\n\
         kubectl{on} --namespace {SYSTEM_NAMESPACE} create secret generic agent-app \\\n  \
         --from-literal=app=<the App's client id> --from-file=key=<the key GitHub gave you>.pem\n\
         kubectl{on} annotate pools.runners.purlis.dev {pool} --overwrite \\\n  \
         runners.purlis.dev/forge-writer=\"$(kubectl{on} auth whoami -o jsonpath='{{.status.userInfo.username}}')\"\n\
         kubectl{on} patch pools.runners.purlis.dev {pool} --type merge \\\n  \
         -p '{{\"spec\":{{\"forge\":{{\"appKeySecretRef\":{{\"name\":\"agent-app\"}},\"repos\":[\"<owner>/*\"],\"mintInCluster\":true}}}}}}'"
    )
}

#[cfg(test)]
mod tests;
