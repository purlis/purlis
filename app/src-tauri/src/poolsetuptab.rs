// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Pool setup tab's commands** (spec #4469, #4480): what `purlis_core::poolsetup` answers, as
//! the window reads it, and the choices the person makes there. Reading changes nothing here;
//! each change happens on its own button.

use crate::dockerpools::PoolAdding;
use crate::planes::{PlaneId, Planes};

/// One step's next action.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupAction {
    pub id: String,
    pub label: String,
    pub does: String,
    pub command: Option<String>,
    pub url: Option<String>,
}

/// A step by hand: the exact commands, and what running them means (#4481).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupSee {
    pub note: String,
    pub text: String,
}

/// One line of what Try it checks (#4483): `ok`, `missing`, `todo` or `skip`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupCheck {
    /// What it checks: `chat`, `signins` or `identity`.
    pub about: String,
    pub verdict: String,
    pub title: String,
    pub more: Option<String>,
}

/// One step as it stands.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupStep {
    pub id: String,
    pub name: String,
    pub state: String,
    pub line: Option<String>,
    pub findings: Vec<String>,
    pub next: Option<PoolSetupAction>,
    pub skippable: bool,
    pub see: Option<PoolSetupSee>,
    pub checks: Vec<PoolSetupCheck>,
    /// A second way on, beside `next`: on Where, the other kind (ADR 0123 §6).
    pub other: Option<PoolSetupAction>,
    /// The machine, on Connect (#6266).
    pub machine: Option<PoolSetupMachine>,
    /// What a finding is, by its key (`pool-pin-changed`, `plain-tcp`, …).
    pub cause: Option<String>,
    /// The choice purlis didn't recommend, where there is one (Where's other place, #4492).
    pub also: Option<PoolSetupAction>,
    /// Install's parts, each with its own button (#4493).
    pub parts: Vec<PoolSetupPart>,
    /// How the step is undone by hand, never by a button: uninstalling from a cluster.
    pub undo: Option<PoolSetupSee>,
    /// What Set up Docker on another machine, or Connect again, opens Connect with (#6268).
    pub prefill: Option<PoolSetupPrefill>,
}

/// **A machine to open Connect with** (#6268): its route, and the Docker context it was read
/// from, where it was read from one.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupPrefill {
    pub route: String,
    pub context: Option<String>,
}

/// A host key, as Connect shows it (#6266).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupKey {
    pub algorithm: String,
    pub fingerprint: String,
}

/// The machine of a pool on another machine, as Connect shows it (#6266).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupMachine {
    pub route: Option<String>,
    /// The key file the route signs in with, where a Podman connection names one (#6269).
    pub identity: Option<String>,
    pub host: Option<String>,
    /// The key it showed at Connect to, to confirm.
    pub key: Option<PoolSetupKey>,
    /// Whether the person's own ssh knows that key for it; null where it knows none.
    pub known: Option<bool>,
    /// The key trusted for it.
    pub trusted: Option<PoolSetupKey>,
    /// The key it shows now, where that isn't the one trusted.
    pub now: Option<PoolSetupKey>,
    /// The engine's id trusted for it.
    pub engine: Option<String>,
    /// **Over Docker's TLS API** (#6267): `tls` for a `tcp://` machine, `ssh` otherwise.
    pub over: String,
    /// The TLS files named under Options.
    pub tls: Option<PoolSetupTlsFiles>,
    /// The CA read at Connect to, to confirm.
    pub ca: Option<PoolSetupCa>,
    /// The CA's fingerprint trusted for it.
    pub trusted_ca: Option<String>,
    /// The fingerprint the CA file holds now, where that isn't the one trusted.
    pub ca_now: Option<String>,
    /// The Docker context the route was read from (#6268).
    pub context: Option<String>,
}

/// The TLS files named for a `tcp://` machine, as Connect's Options show them (#6267).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupTlsFiles {
    pub ca: String,
    pub cert: String,
    pub key: Option<String>,
    pub key_vault: Option<String>,
    pub key_name: Option<String>,
}

/// A CA, as Connect shows it to confirm (#6267).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupCa {
    pub fingerprint: String,
    pub subject: Option<String>,
    pub file: String,
}

/// One part of a cluster's install (#4493): `upstream`, `definitions`, `signing-key` or
/// `operator`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupPart {
    pub id: String,
    pub name: String,
    pub done: bool,
    pub line: String,
    pub next: Option<PoolSetupAction>,
}

/// The kubeconfig's contexts, by name alone, for Where's Options on a cluster (#4492).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupContexts {
    pub names: Vec<String>,
    pub recommended: Option<String>,
    pub chosen: Option<String>,
    /// The chosen cluster's Pools, by name, where its operator runs (#4494).
    pub pools: Vec<String>,
}

/// What the tab draws.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupShown {
    /// The kind of pool this setup makes: `docker`, `cluster` (#4492) or `remote-docker` (#6266).
    pub kind: String,
    /// The pool this setup made.
    pub pool: Option<String>,
    /// The name typed on the Pool step, for the box to hold.
    pub name: Option<String>,
    /// The step the person has reached.
    pub at: Option<String>,
    pub steps: Vec<PoolSetupStep>,
    /// The pool was made by hand under the name typed, and purlis noticed (#4481).
    pub noticed: bool,
    /// Pools this machine keeps that this setup isn't on.
    pub others: Vec<String>,
    /// The name a new pool is recommended, which no pool here has (#6256).
    pub recommended: String,
    pub contexts: PoolSetupContexts,
}

/// What the Pool step's button did: nothing made, and why, where a team's image breaks the
/// contract; none where the pool was made.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupCreated {
    pub breaks: Vec<String>,
}

/// A core value's word, as it serializes.
#[cfg(unix)]
fn word(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|said| said.as_str().map(str::to_owned))
        .unwrap_or_default()
}

#[cfg(unix)]
fn action_shown(next: &purlis_core::poolsetup::Action) -> PoolSetupAction {
    PoolSetupAction {
        id: word(&next.id),
        label: next.label.clone(),
        does: next.does.clone(),
        command: next.command.clone(),
        url: next.url.clone(),
    }
}

/// What the core answered, as the window takes it.
#[cfg(unix)]
pub fn shown(found: purlis_core::poolsetup::Resumed) -> PoolSetupShown {
    let purlis_core::poolsetup::Resumed {
        setup,
        name,
        noticed,
        recommended,
    } = found;
    let key = |key: &purlis_core::poolsetup::connecting::KeyShown| PoolSetupKey {
        algorithm: key.algorithm.clone(),
        fingerprint: key.fingerprint.clone(),
    };
    PoolSetupShown {
        pool: setup.pool.clone(),
        name,
        noticed,
        recommended,
        others: setup.others.clone(),
        kind: word(&setup.kind),
        contexts: PoolSetupContexts {
            names: setup.contexts.names.clone(),
            recommended: setup.contexts.recommended.clone(),
            chosen: setup.contexts.chosen.clone(),
            pools: setup.contexts.pools.clone(),
        },
        at: setup.at.as_ref().map(word),
        steps: setup
            .steps
            .iter()
            .map(|step| PoolSetupStep {
                id: word(&step.id),
                name: step.name.to_owned(),
                state: word(&step.state),
                line: step.line.clone(),
                findings: step.findings.clone(),
                next: step.next.as_ref().map(action_shown),
                also: step.also.as_ref().map(action_shown),
                parts: step
                    .parts
                    .iter()
                    .map(|part| PoolSetupPart {
                        id: word(&part.id),
                        name: part.name.to_owned(),
                        done: part.done,
                        line: part.line.clone(),
                        next: part.next.as_ref().map(action_shown),
                    })
                    .collect(),
                undo: step.undo.as_ref().map(|undo| PoolSetupSee {
                    note: undo.note.clone(),
                    text: undo.text.clone(),
                }),
                other: step.other.as_ref().map(action_shown),
                machine: step.machine.as_ref().map(|machine| PoolSetupMachine {
                    route: machine.route.clone(),
                    identity: machine.identity.clone(),
                    host: machine.host.clone(),
                    key: machine.key.as_ref().map(key),
                    known: machine.known,
                    trusted: machine.trusted.as_ref().map(key),
                    now: machine.now.as_ref().map(key),
                    engine: machine.engine.clone(),
                    over: machine.over.to_owned(),
                    tls: machine.tls.as_ref().map(|tls| PoolSetupTlsFiles {
                        ca: tls.ca.clone(),
                        cert: tls.cert.clone(),
                        key: tls.key.clone(),
                        key_vault: tls.key_vault.clone(),
                        key_name: tls.key_name.clone(),
                    }),
                    ca: machine.ca.as_ref().map(|ca| PoolSetupCa {
                        fingerprint: ca.fingerprint.clone(),
                        subject: ca.subject.clone(),
                        file: ca.file.clone(),
                    }),
                    trusted_ca: machine.trusted_ca.clone(),
                    ca_now: machine.ca_now.clone(),
                    context: machine.context.clone(),
                }),
                prefill: step.prefill.as_ref().map(|prefill| PoolSetupPrefill {
                    route: prefill.route.clone(),
                    context: prefill.context.clone(),
                }),
                cause: step.cause.map(str::to_owned),
                skippable: step.skippable,
                see: step.see.as_ref().map(|see| PoolSetupSee {
                    note: see.note.clone(),
                    text: see.text.clone(),
                }),
                checks: step
                    .checks
                    .iter()
                    .map(|check| PoolSetupCheck {
                        about: word(&check.about),
                        verdict: word(&check.verdict),
                        title: check.title.clone(),
                        more: check.more.clone(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(unix)]
fn config_root() -> Result<std::path::PathBuf, String> {
    purlis_core::machine::config_root()
        .ok_or_else(|| "There is no config home on this machine to keep a pool in.".to_owned())
}

/// **Each step's state, read now**: the pool this setup made, else a new one's.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<PoolSetupShown, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    #[cfg(unix)]
    {
        crate::off_the_window("reading Pool setup", move || {
            use purlis_core::dockerpool::Docker;
            use purlis_core::poolsetup::{self, Here};
            let config = config_root()?;
            let docker = poolsetup::docker_here();
            let kubectl = poolsetup::cluster::kubectl_here();
            let here = Here {
                config: &config,
                plane: Some(&root),
                docker: docker.as_ref(),
                engine: Docker::engine_on_this_machine(),
                kubectl: kubectl.as_ref().map_err(String::as_str),
            };
            Ok(shown(
                poolsetup::resumed(&here).map_err(|why| why.to_string())?,
            ))
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = root;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Check again** (#6266): [`pool_setup`], with every pool on another machine whose forward
/// has been failing tried again and looked at again now, not after its wait.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_check_again(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<PoolSetupShown, String> {
    purlis_core::sshroute::pool::look_again();
    pool_setup(planes, plane).await
}

/// **Keeps that the person chose Docker on Where.** Nothing else changes.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_choose_docker() -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", || {
            purlis_core::poolsetup::choose_kind(
                &config_root()?,
                purlis_core::poolsetup::Kind::Docker,
            )
            .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps that the person chose Docker on another machine on Where** (#6266). Nothing else
/// changes, here or there.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_choose_remote_docker() -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", || {
            purlis_core::poolsetup::choose_kind(
                &config_root()?,
                purlis_core::poolsetup::Kind::RemoteDocker,
            )
            .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps that the person chose their team's cluster on Where**, through kubeconfig context
/// `context` (#4492): its name alone. Nothing on the cluster changes. With no context, only the
/// kind is kept, as Add a cluster pool… opens Pool setup: Where then recommends a context.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_choose_cluster(context: Option<String>) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", move || {
            use purlis_core::poolsetup::{self, Kind};
            let config = config_root()?;
            if let Some(context) = context {
                return poolsetup::choose_cluster(&config, &context).map_err(|why| why.to_string());
            }
            poolsetup::choose_kind(&config, Kind::Cluster).map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = context;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps the machine picked or typed on Connect**, or forgets it (#6266). It contacts
/// nothing.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_machine(
    route: Option<String>,
    identity: Option<String>,
) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", move || {
            purlis_core::poolsetup::choose_machine(
                &config_root()?,
                route.as_deref(),
                identity.as_deref().map(std::path::Path::new),
            )
            .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (route, identity);
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Applies one part of a cluster's install** (#4493), on that part's own button: `upstream`,
/// `definitions`, `signing-key` or `operator`, to the cluster chosen on Where, with the person's
/// own kubectl and sign-in.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_install(part: String) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("installing on your cluster", move || {
            use purlis_core::poolsetup::{self, install::PartId};
            let part: PartId = serde_json::from_value(serde_json::Value::String(part.clone()))
                .map_err(|_| format!("there is no part of the install called {part}"))?;
            let kubectl = poolsetup::cluster::kubectl_here()?;
            poolsetup::install_part(&config_root()?, &kubectl, part).map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = part;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Makes the Pool `name` in the cluster chosen on Where** (#4494), of runner image `image` by
/// digest, on the Pool step's own button, with the person's own kubectl and sign-in.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_cluster_pool(name: String, image: String) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("making a pool in your cluster", move || {
            use purlis_core::poolsetup;
            let kubectl = poolsetup::cluster::kubectl_here()?;
            poolsetup::create_cluster_pool(&config_root()?, &kubectl, &name, &image)
                .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (name, image);
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **The machines the person's SSH settings name**, for Connect to offer (#6266). It contacts
/// nothing.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_ssh_hosts() -> Result<Vec<String>, String> {
    #[cfg(unix)]
    {
        crate::off_the_window("reading your SSH hosts", || {
            let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
            Ok(purlis_core::sshroute::hosts_here(home.as_deref()))
        })
        .await
    }
    #[cfg(not(unix))]
    {
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// A connection of Podman's own, as Connect offers it (#6269): its name, the route to that
/// machine's Podman socket, and the key file it signs in with, where it names one.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupPodmanConnection {
    pub name: String,
    pub route: String,
    pub identity: Option<String>,
}

/// **The person's Podman connections** (`podman system connection list`), for Connect to offer
/// (#6269). It contacts no machine: this machine's `podman` lists what it keeps.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_podman_connections() -> Result<Vec<PoolSetupPodmanConnection>, String> {
    #[cfg(unix)]
    {
        crate::off_the_window("reading your Podman connections", || {
            Ok(purlis_core::sshroute::podman_connections_here()
                .into_iter()
                .map(|one| PoolSetupPodmanConnection {
                    name: one.name,
                    route: one.route.spelled(),
                    identity: one.route.identity().map(|path| path.display().to_string()),
                })
                .collect())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **A context of the person's Docker, as Connect offers it** (#6268): its name, whether it is
/// the one their `docker` uses now, and what its endpoint was read into: `over` `ssh` or `tls`
/// with its `route`, `here` for a socket on this machine (the local kind), or `refused` with
/// why purlis doesn't use it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupDockerContext {
    pub name: String,
    pub current: bool,
    pub over: String,
    pub route: Option<String>,
    pub refused: Option<String>,
}

/// **The person's Docker contexts** (`docker context ls`, then `docker context inspect`), for
/// Connect to offer (#6268). It contacts no machine: this machine's `docker` lists what it keeps.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_docker_contexts() -> Result<Vec<PoolSetupDockerContext>, String> {
    #[cfg(unix)]
    {
        crate::off_the_window("reading your Docker contexts", || {
            use purlis_core::dockercontext::Endpoint;
            let Some(docker) = purlis_core::poolsetup::docker_here() else {
                return Ok(Vec::new());
            };
            Ok(purlis_core::dockercontext::contexts_here(docker.program())
                .into_iter()
                .map(|one| {
                    let route = one.endpoint.route();
                    let (over, refused) = match &one.endpoint {
                        Endpoint::Here => ("here", None),
                        Endpoint::Ssh(_) => ("ssh", None),
                        Endpoint::Tls { .. } => ("tls", None),
                        Endpoint::Refused(why) => ("refused", Some(why.clone())),
                    };
                    PoolSetupDockerContext {
                        name: one.name,
                        current: one.current,
                        over: over.to_owned(),
                        route,
                        refused,
                    }
                })
                .collect())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps the Docker context `name` picked on Connect** (#6268, ADR 0123 §2): read again now,
/// once, into the route its endpoint names, its TLS files and its name; a context on this
/// machine's own socket chooses the local kind. It contacts nothing.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_docker_context(name: String) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", move || {
            let docker = purlis_core::poolsetup::docker_here()
                .ok_or_else(|| "purlis found no docker on this machine.".to_owned())?;
            let contexts = purlis_core::dockercontext::contexts_here(docker.program());
            let context = contexts
                .iter()
                .find(|one| one.name == name)
                .ok_or_else(|| {
                    format!(
                        "Your Docker has no context named {} now. Pick it again from the list.",
                        purlis_core::shown::short(&name)
                    )
                })?;
            purlis_core::poolsetup::choose_docker_context(&config_root()?, context)
                .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = name;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keep the pool as it is** (#6268, ADR 0123 §7): pool `pool` stays on its route, and forgets
/// the Docker context it was read from, so doctor stops comparing them.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_keep_pool_as_it_is(pool: String) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping the pool as it is", move || {
            purlis_core::runners::forget_docker_context(&config_root()?, &pool)
                .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = pool;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps the cluster operator's image named under Install's Options** (#4493), or none.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_operator_image(image: Option<String>) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", move || {
            purlis_core::poolsetup::choose_operator_image(&config_root()?, image.as_deref())
                .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = image;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps the TLS files named under Connect's Options** for a `tcp://` machine (#6267, ADR
/// 0123 §4): the CA, the client certificate, and the key's file or a vault entry of this project's
/// (`key_vault` and `key_name`), or forgets them where `ca` is empty. Their paths and names alone
/// are kept, never what they hold. It contacts nothing.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_tls_files(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    ca: String,
    cert: String,
    key: Option<String>,
    key_vault: Option<String>,
    key_name: Option<String>,
) -> Result<PoolSetupShown, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    #[cfg(unix)]
    {
        crate::off_the_window("keeping the TLS files", move || {
            use purlis_core::tlsroute::{Files, KeyFrom};
            let config = config_root()?;
            let filled = |text: Option<String>| text.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
            // A path typed from the home, as the fields suggest (`~/.docker/ca.pem`), is kept
            // absolute: the core takes nothing else.
            let env = purlis_core::secrets::Env::from_process();
            let path = |typed: &str| purlis_core::secrets::expanduser(typed.trim(), &env);
            let files = if ca.trim().is_empty() {
                None
            } else {
                let key = match (filled(key), filled(key_vault), filled(key_name)) {
                    (Some(key), None, None) => KeyFrom::File(path(&key)),
                    (None, Some(vault), Some(key)) => KeyFrom::Vault {
                        plane: root.clone(),
                        vault,
                        key,
                    },
                    _ => {
                        return Err(
                            "Name your client certificate's key: its file, or the vault and key                              that hold it."
                                .to_owned(),
                        );
                    }
                };
                Some(Files::new(path(&ca), path(&cert), key)?)
            };
            purlis_core::poolsetup::choose_tls_files(&config, files)
                .map_err(|why| why.to_string())?;
            read_shown(&config, &root, None)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (root, ca, cert, key, key_vault, key_name);
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Connect to <machine>** (#6266, ADR 0123 §3): the key the machine Connect keeps shows, met
/// with nothing signed in, and what is read, with it laid over Connect. Nothing is kept.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_connect(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<PoolSetupShown, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    #[cfg(unix)]
    {
        crate::off_the_window("contacting the machine", move || {
            use purlis_core::poolsetup::{self, Pressed};
            let config = config_root()?;
            let chosen = poolsetup::read_choices(&config)
                .map_err(|why| why.to_string())?
                .connect
                .ok_or_else(|| "Pick the machine first.".to_owned())?;
            let docker = poolsetup::docker_here()
                .ok_or_else(|| "purlis found no docker command on this machine.".to_owned())?;
            // Over SSH the key the machine shows; over TLS the CA its files name (#6267).
            let pressed: Pressed = poolsetup::connecting::meet_chosen(&config, &docker, &chosen);
            read_shown(&config, &root, Some(pressed))
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = root;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Trust this machine** (#6266, ADR 0123 §3): the machine met again, its key held to
/// `fingerprint`, the one the person saw, the sign-in and the engine checked, and the pin kept
/// for the Pool step. What went wrong is laid over Connect, and nothing is kept then.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_trust(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    fingerprint: String,
) -> Result<PoolSetupShown, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    #[cfg(unix)]
    {
        crate::off_the_window("checking the machine", move || {
            use purlis_core::poolsetup::{self, Pressed};
            let config = config_root()?;
            let chosen = poolsetup::read_choices(&config)
                .map_err(|why| why.to_string())?
                .connect
                .ok_or_else(|| "Pick the machine first.".to_owned())?;
            let docker = poolsetup::docker_here()
                .ok_or_else(|| "purlis found no docker command on this machine.".to_owned())?;
            match poolsetup::connecting::trust_chosen(&config, &docker, &chosen, &fingerprint)
                .map_err(|why| why.to_string())?
            {
                Ok(()) => read_shown(&config, &root, None),
                Err(finding) => read_shown(&config, &root, Some(Pressed::Found(finding))),
            }
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (root, fingerprint);
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// What Pool setup shows now, with `pressed` laid over Connect.
#[cfg(unix)]
fn read_shown(
    config: &std::path::Path,
    root: &std::path::Path,
    pressed: Option<purlis_core::poolsetup::Pressed>,
) -> Result<PoolSetupShown, String> {
    use purlis_core::dockerpool::Docker;
    use purlis_core::poolsetup::{self, Here};
    let docker = poolsetup::docker_here();
    let kubectl = poolsetup::cluster::kubectl_here();
    let here = Here {
        config,
        plane: Some(root),
        docker: docker.as_ref(),
        engine: Docker::engine_on_this_machine(),
        kubectl: kubectl.as_ref().map_err(String::as_str),
    };
    Ok(shown(
        poolsetup::resumed_pressed(&here, pressed).map_err(|why| why.to_string())?,
    ))
}

/// **Keeps the name typed on the Pool step**, or none, so coming back finds it.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_name(name: Option<String>) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", move || {
            purlis_core::poolsetup::choose_name(&config_root()?, name.as_deref())
                .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = name;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps that this setup goes on with pool `name`**, once it has made it.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_pool(name: String) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", move || {
            purlis_core::poolsetup::choose_pool(&config_root()?, &name)
                .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = name;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Add a pool** (#6256): starts a new pool's setup, unless one was started and not finished.
/// `true` where setup goes on with that one, for the tab to say so with Start over.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_add() -> Result<bool, String> {
    #[cfg(unix)]
    {
        crate::off_the_window("starting a new pool", || {
            purlis_core::poolsetup::start_adding(&config_root()?)
                .map(|found| found == purlis_core::poolsetup::Adding::Continuing)
                .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Start over** (#6256): forgets the unfinished setup, and keeps any pool it made.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_start_over() -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("starting over", || {
            purlis_core::poolsetup::start_over(&config_root()?).map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **The command that does what the Pool step's button would do with `adding`**: See what this
/// does, from the value the button sends (Q30). It reads nothing and changes nothing.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_command(adding: PoolAdding) -> Result<String, String> {
    #[cfg(unix)]
    {
        Ok(adding.asked().command())
    }
    #[cfg(not(unix))]
    {
        let _ = adding;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Makes the pool `adding` asks for**, on the Pool step's own button: what `purlis pool add`
/// runs, a team's image held to the contract first.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_create(adding: PoolAdding) -> Result<PoolSetupCreated, String> {
    #[cfg(unix)]
    {
        crate::off_the_window("adding a pool", move || {
            use purlis_core::poolsetup::asked::{self, Created};
            let machine = crate::runnerlinks::Machine::here(&config_root()?)?;
            let desktop = machine.desktop_made()?.pinned();
            let made = asked::create(&machine.config, &machine.docker, &adding.asked(), &desktop)
                .map_err(|why| crate::dockerpools::refused_said(&why.to_string()))?;
            Ok(PoolSetupCreated {
                breaks: match made {
                    Created::Added => Vec::new(),
                    Created::Breaks(problems) => problems,
                },
            })
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = adding;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// One of this machine's profiles against a pool's limit (#4482).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupProfileSize {
    pub name: String,
    /// What a send to the runner carries, in MiB rounded up, and in files; null where unread.
    pub mib: Option<u32>,
    pub files: Option<u32>,
    /// Why it could not be measured.
    pub unread: Option<String>,
    /// Each limit it is over, in words, with what to raise it to.
    pub over: Vec<PoolSetupOver>,
    /// The project's default, or a profile an open chat runs on.
    pub in_use: bool,
}

/// A limit a profile is over.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupOver {
    /// The limit's key, as `pool_limit_set` takes it.
    pub key: String,
    pub said: String,
    /// What to raise it to, where a value would do.
    pub raise_to: Option<u32>,
    /// What its runner's older purlis holds it to, and what helps, where only that does (#5175).
    pub older_runner: Option<OlderRunner>,
}

/// **What helps a profile over what a pool's older runner takes** (#5175): by what makes its image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum OlderRunner {
    /// A pool of purlis's own image: a new runner on the current image, which the window makes.
    Move,
    /// A pool of a team's own image, which purlis never moves: built anew, and the pool added again.
    OwnImage,
    /// A cluster pool, whose image its Pool in the cluster names.
    Cluster,
}

/// What helps pool `pool`'s older runner, of the pool as this machine keeps it.
#[cfg(unix)]
fn older_runner_way(config: &std::path::Path, pool: &str) -> OlderRunner {
    let docker = purlis_core::runners::pools(config)
        .unwrap_or_default()
        .into_iter()
        .find(|one| one.name() == pool);
    match docker {
        Some(one) if one.image().is_purlis_own() => OlderRunner::Move,
        Some(_) => OlderRunner::OwnImage,
        None => OlderRunner::Cluster,
    }
}

/// This machine's profiles against pool `pool`'s limits (#4482).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupSizes {
    pub most_mib: u32,
    pub most_files: u32,
    pub profiles: Vec<PoolSetupProfileSize>,
}

/// A count as the window takes it.
#[cfg(unix)]
fn small(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// **This machine's profiles against pool `pool`'s limits**, the ones in use in `plane` marked:
/// measured as a send measures them, with no file read, and held to what its runner takes where
/// one link can tell (#4719). It changes nothing.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_sizes(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    pool: String,
) -> Result<PoolSetupSizes, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    #[cfg(unix)]
    {
        crate::off_the_window("measuring your profiles", move || {
            let config = config_root()?;
            // Held to what the pool's runner takes where it can say, as a send to it is (#4719).
            let highest = crate::poolprofiles::runner_speaks(&config, &pool).map_or_else(
                purlis_core::poollimits::Chosen::default,
                purlis_core::profilestore::onrunner::highest_on_a_runner_of,
            );
            let sizes =
                purlis_core::poolsetup::signins::sizes(&config, Some(&root), &pool, &highest)
                    .map_err(|why| why.to_string())?;
            let way = older_runner_way(&config, &pool);
            Ok(PoolSetupSizes {
                most_mib: small(sizes.most_mib),
                most_files: small(sizes.most_files),
                profiles: sizes
                    .profiles
                    .into_iter()
                    .map(|one| PoolSetupProfileSize {
                        name: one.name,
                        mib: one.mib.map(small),
                        files: one.files.map(small),
                        unread: one.unread,
                        over: one
                            .hits
                            .iter()
                            .map(|hit| PoolSetupOver {
                                key: hit.limit.limit().key.to_owned(),
                                said: hit.said(),
                                raise_to: hit.suggested.map(small),
                                older_runner: hit.older_runner.then_some(way),
                            })
                            .collect(),
                        in_use: one.in_use,
                    })
                    .collect(),
            })
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (root, pool);
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **What Sign-ins' button runs, by hand, for the profiles ticked** (#4718, #5174, Q30): `adding`
/// recorded on pool `pool`, each of `taking_off` taken off it, then each of `signing_in` signed in
/// there, as the core spells it. None where the button would run nothing. It only spells.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_sign_ins_by_hand(
    pool: String,
    adding: Vec<String>,
    taking_off: Vec<String>,
    signing_in: Vec<String>,
) -> Result<Option<PoolSetupSee>, String> {
    use purlis_core::poolsetup::signins;
    let names_none = purlis_core::machine::config_root()
        .is_some_and(|config| signins::names_none(&config, &pool));
    Ok(
        signins::by_hand(&pool, &adding, &taking_off, &signing_in, names_none).map(|see| {
            PoolSetupSee {
                note: see.note,
                text: see.text,
            }
        }),
    )
}

/// **Moves pool `pool` to a runner on the current image, as the person pressed** (#5175): this
/// version's image, as a new pool is offered it, made beside the old runner, which keeps its chats
/// until they end ([`purlis_core::pools::follow`]). Whether there is such an image is read here,
/// not as the step is measured, which reads no manifest. A pool of a team's own image is refused,
/// and one on that image already says so. The window's alone.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_move_runner(app: tauri::AppHandle, pool: String) -> Result<(), String> {
    #[cfg(unix)]
    {
        use crate::dockerpools::RunnerImageOffered;
        let image = match crate::poolfollow::image_offered(&app).await? {
            RunnerImageOffered::Ours { image } => image,
            RunnerImageOffered::Newer { version, .. } => {
                return Err(format!(
                    "the pool stays on its runner, because the current runner image is for purlis \
                     {version}. Update purlis, and new chats move to a runner of that version."
                ));
            }
            RunnerImageOffered::NoneNamed => {
                return Err(
                    "the pool stays on its runner, because this version's release has no runner \
                     image. Update purlis when a newer version is out."
                        .to_owned(),
                );
            }
            RunnerImageOffered::LocalBuild => {
                return Err(
                    "the pool stays on its runner, because a local build of purlis names no \
                     runner image. Run a released purlis to move it."
                        .to_owned(),
                );
            }
        };
        crate::off_the_window("moving a pool to this version", move || {
            let image = purlis_core::dockerpool::Image::pinned(&image).map_err(|why| why.0)?;
            let machine = crate::runnerlinks::Machine::here(&config_root()?)?;
            let desktop = machine.desktop()?.pinned();
            let docker = machine.for_pool(&pool)?.docker;
            match purlis_core::pools::follow(&machine.config, &docker, &pool, &image, &desktop)
                .map_err(|why| crate::dockerpools::refused_said(&why.to_string()))?
            {
                Some(_) => Ok(()),
                None => Err(format!(
                    "pool {pool} is on this version's image already, so a new runner would be no \
                     newer. Remove the pool in Settings › Pools and add it again."
                )),
            }
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, pool);
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps that the person skipped step `step` on pool `pool`, or takes the skip back.** Refused
/// for a step the ones after it need.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_skip(pool: String, step: String, skipped: bool) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", move || {
            use purlis_core::poolsetup::{self, StepId};
            let step: StepId = serde_json::from_value(serde_json::Value::String(step))
                .map_err(|_| "Pool setup has no step of that name.".to_owned())?;
            let config = config_root()?;
            if skipped {
                poolsetup::skip(&config, &pool, step)
            } else {
                poolsetup::unskip(&config, &pool, step)
            }
            .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (pool, step, skipped);
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Starts pool `pool`'s runner again**, on the Sign-ins step's own button: one a day without a
/// link stopped its own container, which is checked as it was made before it starts, as a link to
/// it would start it. It goes back to sleep by itself.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_wake(pool: String) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("waking the runner", move || {
            let machine = crate::runnerlinks::Machine::here(&config_root()?)?;
            let kept = purlis_core::runners::runner_for(&machine.config, &pool)
                .map_err(|why| why.to_string())?;
            let docker = machine.for_pool(&pool)?.docker;
            crate::runnerlinks::checked_or_woken(&docker, &machine.config, &kept, true)
                .map_err(|why| why.0)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = pool;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// Why Try it didn't pass, and the step that mends it, where one does (#4483).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupFailed {
    /// What happened and what to do, in a sentence or two.
    pub why: String,
    /// The step to go to, by its id: `signins` for a sign-in. Null where trying again is it.
    pub step: Option<String>,
}

/// What Try it found.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupTried {
    /// Why it didn't pass; null where a real chat started and the proof is kept, or where the
    /// person cancelled it.
    pub failed: Option<PoolSetupFailed>,
    /// The person cancelled it (#4976): nothing was kept, and its chat is closed.
    pub cancelled: bool,
    /// What Try it proved of the agent's identity itself (#4490), for the identity's line: a
    /// read-only token GitHub made for the App, read with and revoked. None where the identity
    /// step's own line stands: no App of Agent identity's, no repo, or a step not done.
    pub identity: Option<PoolSetupCheck>,
    /// The workspace its chat started in, where it got that far: where Open a chat opens.
    pub workspace: Option<String>,
}

/// **How long Try it waits for its chat's harness to report its start** once the chat is under
/// way, as long as a grade waits for its chat's (`skillgrade`'s `A_CHAT_BEGINS_WITHIN`). A chat
/// that waits on something no hook tells of, such as a question at its launch, has spent nothing
/// by then.
#[cfg(unix)]
const TRY_BEGINS_WITHIN: std::time::Duration = std::time::Duration::from_secs(120);

/// How often Try it looks whether its chat has reported its start.
#[cfg(unix)]
const TRY_LOOKS_EVERY: std::time::Duration = std::time::Duration::from_millis(500);

/// The size Try it's chat runs at: it is closed before anyone reads it.
#[cfg(unix)]
const TRY_SIZE: purlis_core::engine::Size = purlis_core::engine::Size {
    columns: 120,
    rows: 40,
};

/// A Try it's own Cancel (#4976).
#[cfg(unix)]
type Cancel = std::sync::Arc<std::sync::atomic::AtomicBool>;

/// The pools a Try it is under way on, each with its own Cancel (#4976), so a second press waits
/// for the first and a Cancel stops only the try it was pressed on.
#[cfg(unix)]
static TRYING: std::sync::Mutex<std::collections::BTreeMap<String, Cancel>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// **Try it** (#4483, Q32): starts a real chat on pool `pool`, on the Try it step's own button,
/// waits for the hook its harness reports its session's start by (ADR 0076), closes it, and keeps
/// that it started (`purlis_core::poolsetup::tried`). Nothing is typed into it. A start that is
/// refused, or a chat that never reports its start, comes back as `failed`, with the step that
/// mends it where one does.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_try(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    pool: String,
    workspace: Option<String>,
) -> Result<PoolSetupTried, String> {
    let held = planes.held(&plane)?;
    #[cfg(unix)]
    {
        crate::off_the_window("trying the pool", move || {
            let docker = purlis_core::poolsetup::docker_here();
            try_it(
                &held,
                &config_root()?,
                docker.as_ref(),
                &pool,
                workspace.as_deref(),
            )
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (held, pool, workspace);
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// [`pool_setup_try`]'s work, off the window: Try it on pool `pool` of the project `held`, with
/// this machine's config home `config` and its `docker`, where it has one. What the parity suite
/// drives on a real Docker pool (`behaviour::pool_setup_try`, #4977).
#[cfg(unix)]
pub(crate) fn try_it(
    held: &std::sync::Arc<crate::planes::Held>,
    config: &std::path::Path,
    docker: Option<&purlis_core::dockerpool::Docker>,
    pool: &str,
    workspace: Option<&str>,
) -> Result<PoolSetupTried, String> {
    purlis_core::runners::usable_pool_name(pool)?;
    let Some(trying) = Trying::on(pool) else {
        return Err(format!("Try it is already under way on {pool}."));
    };
    try_on(held, config, docker, pool, workspace, &trying)
}

/// **Cancels the Try it under way on pool `pool`** (#4976), on the person's press, as Cancel on
/// the picker stops a start: its start stops waiting for the runner, its wait for the chat's start
/// ends, and its chat is closed. Nothing is kept. Silent where no Try it is under way there.
#[tauri::command]
#[specta::specta]
pub fn pool_setup_try_cancel(pool: String) {
    #[cfg(unix)]
    {
        if cancel_trying(&pool) {
            tracing::info!("purlis: the person cancelled Try it on pool {pool}");
        }
    }
    #[cfg(not(unix))]
    {
        let _ = pool;
    }
}

/// Tells the Try it on `pool` to stop: whether one was under way there.
#[cfg(unix)]
fn cancel_trying(pool: &str) -> bool {
    let trying = TRYING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    trying.get(pool).is_some_and(|cancel| {
        cancel.store(true, std::sync::atomic::Ordering::SeqCst);
        true
    })
}

/// **A Try it under way on one pool**: held for as long as it runs, and let go however it ends,
/// a panic too, so a second press waits for the first and a failed one never locks the pool.
/// It holds its own Cancel, never the picker's: a Cancel on a chat being started elsewhere in
/// the project leaves it alone, and its own leaves those alone.
#[cfg(unix)]
struct Trying(String, Cancel);

#[cfg(unix)]
impl Trying {
    /// Holds `pool`, or none where a Try it is already under way on it.
    fn on(pool: &str) -> Option<Trying> {
        let mut trying = TRYING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if trying.contains_key(pool) {
            return None;
        }
        let cancel = Cancel::default();
        trying.insert(pool.to_owned(), std::sync::Arc::clone(&cancel));
        Some(Trying(pool.to_owned(), cancel))
    }

    /// Whether the person cancelled it.
    fn cancelled(&self) -> bool {
        self.1.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(unix)]
impl Drop for Trying {
    fn drop(&mut self) {
        TRYING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.0);
    }
}

/// **Try it's chat, ended however Try it ends**: unwatched and closed on drop, a panic too, so
/// no chat it started is left running.
#[cfg(unix)]
struct TryChat<'a> {
    held: &'a std::sync::Arc<crate::planes::Held>,
    number: u32,
    session: Option<u32>,
}

#[cfg(unix)]
impl Drop for TryChat<'_> {
    fn drop(&mut self) {
        self.held.hooks().skill_runs().unwatch(self.number);
        if let Some(session) = self.session
            && let Err(why) = self.held.close_chat(session)
        {
            tracing::warn!("purlis: Try it's chat {session} did not close ({why})");
        }
    }
}

/// A failure of Try it, as the window takes it.
#[cfg(unix)]
fn failed(why: String, step: Option<purlis_core::poolsetup::StepId>) -> PoolSetupTried {
    PoolSetupTried {
        failed: Some(PoolSetupFailed {
            why,
            step: step.as_ref().map(word),
        }),
        cancelled: false,
        identity: None,
        workspace: None,
    }
}

/// **The longest Try it waits for its runner to start its chat**, Cancel or not: far past a
/// runner woken and an image pulled, and short of a try nobody can end.
#[cfg(unix)]
const TRY_STARTS_WITHIN: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// **Cancels a try once `within` has passed**, and says so, unless the try ended first: dropped,
/// it lets its waiting thread go at once.
#[cfg(unix)]
struct Backstop {
    ran_out: std::sync::Arc<std::sync::atomic::AtomicBool>,
    done: Option<std::sync::mpsc::Sender<()>>,
}

#[cfg(unix)]
impl Backstop {
    fn after(within: std::time::Duration, cancel: &Cancel) -> Backstop {
        use std::sync::atomic::Ordering;
        let ran_out = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (done, ended) = std::sync::mpsc::channel::<()>();
        let (flag, cancel) = (
            std::sync::Arc::clone(&ran_out),
            std::sync::Arc::clone(cancel),
        );
        let spawned = std::thread::Builder::new()
            .name("purlis-try-backstop".into())
            .spawn(move || {
                if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = ended.recv_timeout(within)
                {
                    flag.store(true, Ordering::SeqCst);
                    cancel.store(true, Ordering::SeqCst);
                }
            });
        if let Err(why) = spawned {
            tracing::warn!("purlis: Try it runs without its backstop ({why})");
        }
        Backstop {
            ran_out,
            done: Some(done),
        }
    }

    fn ran_out(&self) -> bool {
        self.ran_out.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(unix)]
impl Drop for Backstop {
    fn drop(&mut self) {
        // Its thread hears the end of the try as the channel closes.
        self.done.take();
    }
}

/// A Try it the person cancelled.
#[cfg(unix)]
const CANCELLED: PoolSetupTried = PoolSetupTried {
    failed: None,
    cancelled: true,
    identity: None,
    workspace: None,
};

/// Why Try it's chat didn't report its start.
#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotStarted {
    /// The person cancelled the try.
    Cancelled,
    /// What happened, and what to do.
    Failed(String),
}

/// **The step that mends a start the pool refused**: Sign-ins, where its runner found the
/// profile signed out (`purlis_core::profilestore::sign_in::is_refusal`); else none.
#[cfg(unix)]
fn step_of_a_refused_start(why: &str) -> Option<purlis_core::poolsetup::StepId> {
    purlis_core::profilestore::sign_in::is_refusal(why)
        .then_some(purlis_core::poolsetup::StepId::Signins)
}

/// **The workspace Try it starts its chat in, and its folder**: workspace `asked`, where the
/// tab is filed in one, else the project's first workspace by name. None where the project has
/// none.
#[cfg(unix)]
fn workspace_to_try(
    root: &std::path::Path,
    asked: Option<&str>,
) -> Option<(String, std::path::PathBuf)> {
    // A folder of the project's own, reached without a link: what a branch folder is held to.
    let found = |name: &str| {
        purlis_core::worktree::confine::workspace_dir(root, name)
            .ok()
            .map(|dir| (name.to_owned(), dir))
    };
    if let Some(chosen) = asked.and_then(found) {
        return Some(chosen);
    }
    let mut names: Vec<String> = std::fs::read_dir(root.join("workspaces"))
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.iter().find_map(|name| found(name))
}

/// A wait in words: `2 minutes`, or `30 seconds` under a minute.
#[cfg(unix)]
fn said_as_time(wait: std::time::Duration) -> String {
    use purlis_core::counts::counted;
    match wait.as_secs() {
        secs @ 0..60 => counted(secs, "second", "seconds"),
        secs => counted(secs.div_ceil(60), "minute", "minutes"),
    }
}

/// **Waits for the chat `heard` tells of to report its session's start** (ADR 0076): its
/// harness's `SessionStart` hook, and nothing it prints. Why it didn't, said for pool `pool`,
/// where it ended first, its program was found gone (`still_running`, asked each look: a program
/// that exits reports no end of its own), or `within` passed; or that the person cancelled
/// (`cancelled`, asked each look, #4976).
#[cfg(unix)]
fn waited_for_its_start(
    heard: &std::sync::mpsc::Receiver<crate::skillrun::Watched>,
    pool: &str,
    within: std::time::Duration,
    still_running: impl Fn() -> bool,
    cancelled: impl Fn() -> bool,
) -> Result<(), NotStarted> {
    use crate::skillrun::Watched;
    use std::sync::mpsc::RecvTimeoutError;
    let began = std::time::Instant::now();
    let ended = || {
        NotStarted::Failed(format!(
            "The chat on {pool} ended before it had started. Open a chat there yourself to see \
             what it says, then try again."
        ))
    };
    loop {
        if cancelled() {
            return Err(NotStarted::Cancelled);
        }
        match heard.recv_timeout(TRY_LOOKS_EVERY.min(within)) {
            Ok(Watched::Began) => return Ok(()),
            Ok(Watched::Closed) | Err(RecvTimeoutError::Disconnected) => return Err(ended()),
            Ok(_) | Err(RecvTimeoutError::Timeout) if !still_running() => return Err(ended()),
            Ok(_) | Err(RecvTimeoutError::Timeout) => {
                if began.elapsed() >= within {
                    return Err(NotStarted::Failed(format!(
                        "The chat on {pool} didn't start within {}. Its harness may be waiting on \
                         a question, so open a chat there yourself and answer it, then try again.",
                        said_as_time(within)
                    )));
                }
            }
        }
    }
}

/// [`try_it`]'s work, with the pool held for it.
#[cfg(unix)]
fn try_on(
    held: &std::sync::Arc<crate::planes::Held>,
    config: &std::path::Path,
    docker: Option<&purlis_core::dockerpool::Docker>,
    pool: &str,
    workspace: Option<&str>,
    trying_now: &Trying,
) -> Result<PoolSetupTried, String> {
    use purlis_core::poolsetup::{self, Here, trying};
    use purlis_core::reopen::Chat;

    let root = held.root().to_path_buf();
    let here = Here {
        config,
        plane: Some(&root),
        docker,
        engine: purlis_core::dockerpool::Docker::engine_on_this_machine(),
        // Try it asks nothing of a cluster: a cluster pool's chat start answers for its runner.
        kubectl: Err("not asked"),
    };
    let facts = poolsetup::read_pool(&here, pool).map_err(|why| why.to_string())?;
    let (set, _check) = purlis_core::profiles::for_launch(&root);
    let declared = purlis_core::harness_declaration::read(&root);
    let candidates: Vec<trying::Candidate> = set
        .profiles()
        .iter()
        .map(|profile| trying::Candidate {
            name: profile.name.clone(),
            reports_its_start: declared
                .get(&profile.kind)
                .and_then(purlis_core::harness_declaration::Declaration::ready_to_type)
                == Some(purlis_core::harness::ReadyToType::WhenItReportsItsStart),
        })
        .collect();
    let profile =
        match trying::profile_to_try(pool, &facts.sign_ins, set.default.as_deref(), &candidates) {
            Ok(profile) => profile,
            Err(not) => return Ok(failed(not.why, not.step)),
        };

    // A chat at the project's root runs on this machine, so Try it starts in a workspace.
    let Some((workspace, cwd)) = workspace_to_try(&root, workspace) else {
        return Ok(failed(
            "Try it starts its chat in a workspace, and this project has none yet. Add a \
             workspace, then try again."
                .to_owned(),
            None,
        ));
    };
    let number = held.chats().sessions().deal();
    let name = number.to_string();
    let start = purlis_core::start::Start {
        profile: Some(profile.clone()),
        persona: None,
        name: name.clone(),
        cwd: Some(cwd),
        resume: None,
        show_footer: false,
        resuming: None,
        // No opt-out of its own: sandboxed as the person's own new chat would be.
        without_sandbox: None,
        held: None,
        grants: Default::default(),
        // The person's press on Try it, in the window.
        made: purlis_core::sandbox::Made::Window,
    };
    // Readied as the pool's runner judges it, since the runner composes it (#2446): on a Linux
    // desktop a sandboxed Claude Code chat runs only on a sandbox-only profile, which no pool
    // keeps, and on the runner it runs wrapped on the profile the Sign-ins step kept.
    let ready = match purlis_core::start::ready_for_a_runner(&start, &root) {
        Ok(ready) => ready,
        Err(why) => return Ok(failed(crate::asking::start_refused(held, why), None)),
    };
    let chat = Chat {
        program: ready.program.clone(),
        args: Vec::new(),
        cwd: ready.cwd.clone(),
        name,
        resume: ready.session.clone(),
        active: false,
        profile: Some(profile),
        persona: None,
        show_footer: false,
        pinned: false,
        number: Some(number),
        label: Some(format!("Try it on {pool}")),
        from: None,
        renamed_from: None,
        // On this pool, whatever the project's or its workspaces' own setting says (#4536).
        runs_on: Some(purlis_core::machine::StartChoice::Pool(pool.to_owned())),
        // It lives for seconds: a relaunch after a quit mid-try never puts it back (#4976).
        unrecorded: true,
        ..Default::default()
    };
    // Watched before it starts, so its first hook is never missed.
    let heard = held.hooks().skill_runs().watch(number);
    let mut chat_held = TryChat {
        held,
        number,
        session: None,
    };
    if trying_now.cancelled() {
        return Ok(CANCELLED);
    }
    // A start the person can cancel waits for its runner without a limit of its own, so Try it
    // keeps one: a page reloaded mid-try has no Cancel to press, and a runner that keeps its link
    // and never answers would hold the pool's try for good.
    let backstop = Backstop::after(TRY_STARTS_WITHIN, &trying_now.1);
    // Its own Cancel stops the start as the picker's does (#4976): the wait for the runner ends,
    // and a chat that comes up after is closed.
    let started = crate::hostedsessions::cancelled_by(std::sync::Arc::clone(&trying_now.1), || {
        held.chats().start_ready(&chat, &ready, TRY_SIZE)
    });
    let ran_out = || {
        failed(
            format!(
                "The runner of {pool} didn't answer within {}. Check the Pool step, then try \
                 again.",
                said_as_time(TRY_STARTS_WITHIN)
            ),
            None,
        )
    };
    let session = match started {
        Ok(session) => session,
        Err(_) if backstop.ran_out() => return Ok(ran_out()),
        Err(_) if trying_now.cancelled() => return Ok(CANCELLED),
        Err(why) => {
            let step = step_of_a_refused_start(&why);
            return Ok(failed(crate::asking::start_refused(held, why), step));
        }
    };
    chat_held.session = Some(session);
    tracing::info!("purlis: Try it started chat {number} on pool {pool}");
    let ended = waited_for_its_start(
        &heard,
        pool,
        TRY_BEGINS_WITHIN,
        || held.chats().sessions().running().contains(&session),
        || trying_now.cancelled(),
    )
    .err();
    drop(chat_held);
    match ended {
        Some(NotStarted::Cancelled) if backstop.ran_out() => return Ok(ran_out()),
        Some(NotStarted::Cancelled) => return Ok(CANCELLED),
        Some(NotStarted::Failed(why)) => return Ok(failed(why, None)),
        None => {}
    }
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    poolsetup::tried(config, pool, at).map_err(|why| {
        format!("A chat started on {pool}, and purlis couldn't keep that it did: {why}")
    })?;
    Ok(PoolSetupTried {
        failed: None,
        cancelled: false,
        identity: proved_identity(&here, &root, pool),
        workspace: Some(workspace),
    })
}

/// **What Try it proves of the agent's identity** (#4490), where the identity step reads as set
/// and the project has a repo: GitHub makes the App a token narrowed to reading metadata, one repo
/// is read with it, and it is revoked (`agentidentity::prove`, Finish's own proof). It runs in
/// this process, with the App's key read only to mint, and nothing of the token or the key goes
/// anywhere. Only an App Agent identity set up is proved: a hand-written line is never signed
/// with. It doesn't run `gh` or a push in the chat itself.
#[cfg(unix)]
fn proved_identity(
    here: &purlis_core::poolsetup::Here<'_>,
    root: &std::path::Path,
    pool: &str,
) -> Option<PoolSetupCheck> {
    use purlis_core::forge::agentidentity;
    use purlis_core::poolsetup::{CheckOf, StepId, Verdict};
    let setup = purlis_core::poolsetup::read(here, Some(pool)).ok()?;
    let line = setup
        .step(StepId::Try)?
        .checks
        .iter()
        .find(|check| check.about == CheckOf::Identity)?;
    if line.verdict != Verdict::Ok {
        return None;
    }
    // The App behind the very persona and tool the step reads as set, and only a line Agent
    // identity wrote: a proof never stands in for another identity than the line it replaces.
    let (persona, tool) = purlis_core::poolsetup::identity_set_in(root)?;
    if tool != "gh" {
        return None;
    }
    let app = agentidentity::held(root)
        .ok()?
        .into_iter()
        .find(|held| held.ours() && held.personas.contains(&persona))?
        .app;
    let ctx = purlis_core::secrets::Ctx::new(root, purlis_core::secrets::Env::from_process());
    let check = |verdict: Verdict, title: String, more: String| PoolSetupCheck {
        about: word(&CheckOf::Identity),
        verdict: word(&verdict),
        title,
        more: Some(more),
    };
    Some(match agentidentity::prove(&ctx, &app) {
        Ok(proof) => check(
            Verdict::Ok,
            match &proof.read {
                Some(repo) => format!(
                    "The agent's identity read {repo} on GitHub as {}",
                    proof.stand.acts_as()
                ),
                None => format!(
                    "The agent's identity got a token from GitHub as {}",
                    proof.stand.acts_as()
                ),
            },
            "purlis made a read-only token for it and revoked it at once. It doesn't run gh or \
             git push in the chat itself yet."
                .to_owned(),
        ),
        Err(why) => {
            let why = purlis_core::shown::short(why.trim());
            let why = if why.ends_with(['.', '!', '?']) {
                why
            } else {
                format!("{why}.")
            };
            check(
                Verdict::Missing,
                "GitHub didn't confirm the agent's identity".to_owned(),
                format!("{why} Open Agent identity to see how it stands."),
            )
        }
    })
}

/// One pool as the chat picker offers it (#4484, D on #4469).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolInPicker {
    pub pool: String,
    /// False where a chat can't start there: the picker doesn't offer it.
    pub offered: bool,
    /// "Setup not finished", and why: where setup isn't finished, and where a chat can't start.
    pub said: Option<String>,
}

/// **How each pool this machine keeps stands in the chat picker of `plane`** (#4484): read from
/// what this machine keeps and the plane's personas, with nothing asked of Docker or a runner.
/// It changes nothing.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_in_the_picker(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<PoolInPicker>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    #[cfg(unix)]
    {
        crate::off_the_window("reading how your pools stand", move || {
            use purlis_core::poolsetup::{InThePicker, in_the_picker};
            let config = config_root()?;
            let pools = purlis_core::runners::pool_names(&config).map_err(|why| why.to_string())?;
            Ok(in_the_picker(&config, Some(&root), &pools)
                .into_iter()
                .map(|(pool, standing)| {
                    let (offered, said) = match standing {
                        InThePicker::Offered => (true, None),
                        InThePicker::NotFinished(said) => (true, Some(said)),
                        InThePicker::NotOffered(said) => (false, Some(said)),
                    };
                    PoolInPicker {
                        pool,
                        offered,
                        said,
                    }
                })
                .collect())
        })
        .await
    }
    // No pool runs here.
    #[cfg(not(unix))]
    {
        let _ = root;
        Ok(Vec::new())
    }
}

/// **Opens the Docker app**, on the person's press, where this system has one to open. Refused
/// elsewhere, with what to do instead.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_open_docker() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        crate::off_the_window("opening Docker", || {
            let opened = purlis_core::forklock::output(
                std::process::Command::new("/usr/bin/open").args(["-a", "Docker"]),
            )
            .map_err(|why| why.to_string())?;
            if opened.status.success() {
                Ok(())
            } else {
                Err("purlis couldn't open Docker. Open it the way you usually do.".to_owned())
            }
        })
        .await
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("purlis can't open Docker here. Start it the way you usually do.".to_owned())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::sync::mpsc::channel;
    use std::time::Duration;

    use super::{NotStarted, said_as_time, step_of_a_refused_start, waited_for_its_start};
    use crate::skillrun::Watched;
    use purlis_core::poolsetup::StepId;

    const RUNNING: fn() -> bool = || true;
    const NOT_CANCELLED: fn() -> bool = || false;

    #[test]
    fn try_it_passes_on_its_chats_start_hook_and_nothing_before_it() {
        let (tell, heard) = channel();
        tell.send(Watched::TurnEnded).unwrap();
        tell.send(Watched::Began).unwrap();
        assert_eq!(
            waited_for_its_start(
                &heard,
                "dev-box",
                Duration::from_secs(5),
                RUNNING,
                NOT_CANCELLED
            ),
            Ok(())
        );
    }

    #[test]
    fn a_chat_that_ends_before_it_starts_fails_try_it_at_once() {
        let (tell, heard) = channel();
        tell.send(Watched::Closed).unwrap();
        let why = failed_with(waited_for_its_start(
            &heard,
            "dev-box",
            Duration::from_secs(60),
            RUNNING,
            NOT_CANCELLED,
        ));
        assert!(why.contains("ended before it had started"), "{why}");
    }

    #[test]
    fn a_chat_that_never_reports_its_start_fails_try_it_once_the_wait_is_over() {
        let (_tell, heard) = channel();
        let why = failed_with(waited_for_its_start(
            &heard,
            "dev-box",
            Duration::from_millis(50),
            RUNNING,
            NOT_CANCELLED,
        ));
        assert!(why.contains("didn't start within"), "{why}");
    }

    #[test]
    fn a_chat_whose_program_is_gone_fails_try_it_at_once_and_says_it_ended() {
        let (_tell, heard) = channel();
        let began = std::time::Instant::now();
        let why = failed_with(waited_for_its_start(
            &heard,
            "dev-box",
            Duration::from_secs(60),
            || false,
            NOT_CANCELLED,
        ));
        assert!(why.contains("ended before it had started"), "{why}");
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "it waited out the minute"
        );
    }

    fn failed_with(waited: Result<(), NotStarted>) -> String {
        match waited {
            Err(NotStarted::Failed(why)) => why,
            other => panic!("it didn't fail: {other:?}"),
        }
    }

    #[test]
    fn a_cancel_ends_try_its_wait_at_once_and_is_no_failure() {
        let (_tell, heard) = channel();
        let began = std::time::Instant::now();
        assert_eq!(
            waited_for_its_start(&heard, "dev-box", Duration::from_secs(60), RUNNING, || true),
            Err(NotStarted::Cancelled)
        );
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "it waited out the minute"
        );
    }

    #[test]
    fn a_try_cancelled_on_one_pool_leaves_a_try_on_another_running() {
        let (one, other) = (
            super::Trying::on("one").unwrap(),
            super::Trying::on("other").unwrap(),
        );
        assert!(
            super::Trying::on("one").is_none(),
            "a second press runs alongside the first"
        );
        assert!(super::cancel_trying("one"));
        assert!(one.cancelled());
        assert!(!other.cancelled());
        assert!(
            !super::cancel_trying("nowhere"),
            "there was no try to cancel"
        );
    }

    #[test]
    fn a_try_s_backstop_cancels_it_once_its_time_is_out_and_says_it_ran_out() {
        let cancel = super::Cancel::default();
        let backstop = super::Backstop::after(Duration::from_millis(20), &cancel);
        let began = std::time::Instant::now();
        while !cancel.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(
                began.elapsed() < Duration::from_secs(10),
                "it never ran out"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(backstop.ran_out());
    }

    #[test]
    fn a_try_that_ends_first_is_never_cancelled_by_its_backstop() {
        let cancel = super::Cancel::default();
        drop(super::Backstop::after(Duration::from_millis(50), &cancel));
        std::thread::sleep(Duration::from_millis(150));
        assert!(!cancel.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn a_start_its_runner_found_signed_out_sends_the_person_to_sign_ins() {
        let refused = "work is not signed in on the runner of pool dev-box. Nothing was started.";
        assert_eq!(step_of_a_refused_start(refused), Some(StepId::Signins));
        assert_eq!(step_of_a_refused_start("the runner did not answer"), None);
    }

    #[test]
    fn a_wait_is_said_in_words_with_its_unit_counted() {
        assert_eq!(said_as_time(Duration::from_secs(120)), "2 minutes");
        assert_eq!(said_as_time(Duration::from_secs(60)), "1 minute");
        assert_eq!(said_as_time(Duration::from_secs(1)), "1 second");
    }
}
