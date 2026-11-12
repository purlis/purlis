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
        let (namespace, name) = doctor::UPSTREAM_CONTROLLER;
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
    facts.installed = Some(installed(&on));
    if !facts.operator_runs() {
        facts.doctor = Some(doctor(&Asked {
            kubectl: &on,
            context: &context,
            server: server_of(&on),
        }));
    }
    facts
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
    match &cluster.installed {
        Some(Ok(_)) => {
            let parts = parts_of(facts, choices);
            if parts.iter().all(|part| part.done) {
                return Reading::done("Installed");
            }
            Reading::to_do(
                "Install puts purlis's cluster operator on the cluster, a part at a time. Each \
                 button says what it applies, and nothing changes until you press it.",
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
fn doctor_by_hand(on: &str) -> String {
    let (namespace, controller) = doctor::UPSTREAM_CONTROLLER;
    let mut lines = vec![format!("kubectl{on} version")];
    for (name, _) in doctor::UPSTREAM_DEFINITIONS {
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

/// **The Pool step on a cluster, before #4494's screen**: what comes once the install is done.
pub(super) fn pool_reading(facts: &Facts) -> Reading {
    if facts.pool.is_some() {
        return super::pool_reading(facts);
    }
    Reading::to_do(
        "A pool is a named place your chats run. On a cluster, each chat's runner is a pod the \
         cluster operator makes from it.",
        None,
    )
}

#[cfg(test)]
mod tests;
