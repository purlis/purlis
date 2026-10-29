// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A cluster pool, added and removed from the window** (#3292, spec #2041): a Pool of purlis's
//! cluster operator in a team's cluster, reached through this machine's own `kubectl`, found by
//! its absolute path ([`kubectl`]) and pointed at the kubeconfig and context the pool names on
//! every run. Every rule is the core's ([`purlis_core::pools::add_cluster`],
//! [`purlis_core::clusterpool`]); this says what is kept and does what the person pressed.
//!
//! Adding one is two presses in the Add a pool dialog: **Check its key** reads the cluster
//! operator key the cluster's Pool names ([`cluster_pool_key`]), which the dialog shows for the
//! person to check against the one their cluster's admin gives them; **Add pool** pins exactly
//! that key, refused where the Pool names another by then, and claims, pairs and keeps its
//! runner ([`add_cluster_pool`]). The window's alone (`ui::WINDOW_ONLY`), as a Docker pool is.

use std::path::{Path, PathBuf};

use purlis_core::clusterpool::Kubectl;
use purlis_core::runners::{ClusterKept, ClusterPool};

use crate::dockerpools::PoolLink;
use crate::runnerlinks::ToARunner;

/// One cluster pool, as its row draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ClusterPoolRow {
    pub name: String,
    /// The kubeconfig's context its kubectl is pointed at.
    pub context: String,
    /// The cluster's Pool its runner is claimed from.
    pub pool: String,
    /// The namespace its claim is made in.
    pub namespace: String,
    /// The cluster operator key pinned as it was added, spelled `ed25519:<64 hex>`.
    pub operator_key: String,
}

/// `pool`, as its row draws it.
pub fn row(pool: &ClusterPool) -> ClusterPoolRow {
    ClusterPoolRow {
        name: pool.name().to_owned(),
        context: pool.context().to_owned(),
        pool: pool.pool().to_owned(),
        namespace: pool.namespace().to_owned(),
        operator_key: pool.operator().spelled(),
    }
}

/// The contexts of a kubeconfig, as the Add a pool dialog offers them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ClusterContexts {
    /// The kubeconfig read, by its absolute path.
    pub kubeconfig: String,
    pub contexts: Vec<String>,
    /// The context it is on, where it names one.
    pub current: Option<String>,
}

/// What Check its key read of a cluster's Pool.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ClusterPoolKey {
    /// The cluster operator key the Pool's status names, spelled as it gives it.
    pub key: String,
    /// The namespace the context names, where the pool's claim would be made.
    pub namespace: String,
}

/// A cluster pool as the person typed it, with the key they checked.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, specta::Type)]
pub struct ClusterPoolAdding {
    pub name: String,
    /// The kubeconfig, by its path; empty or none is kubectl's own default, `~/.kube/config`.
    pub kubeconfig: Option<String>,
    pub context: String,
    /// The cluster's Pool.
    pub pool: String,
    /// The cluster operator key Check its key showed, spelled as it showed it.
    pub key: String,
}

/// **This machine's `kubectl`, by its absolute path** (#3292,
/// [`Kubectl::on_this_machine`]): where its installers put it, else an absolute folder of the
/// window's `PATH`.
pub fn kubectl() -> Result<Kubectl, String> {
    let path = purlis_core::envvar::var("PATH");
    let home = home();
    Kubectl::on_this_machine(path.as_deref(), home.as_deref()).map_err(|why| why.to_string())
}

fn home() -> Option<PathBuf> {
    purlis_core::envvar::var_os("HOME").map(PathBuf::from)
}

/// **The kubeconfig `typed` names**, by its absolute path: kubectl's own default,
/// `~/.kube/config`, where nothing is typed, and `~/` read as the home folder. Never one taken
/// from an environment: the pool names it on every run.
pub fn kubeconfig_of(typed: Option<&str>, home: Option<&Path>) -> Result<PathBuf, String> {
    let typed = typed.map(str::trim).filter(|typed| !typed.is_empty());
    let no_home = || "There is no home folder to find the kubeconfig in.".to_owned();
    let path = match typed {
        None => home.ok_or_else(no_home)?.join(".kube/config"),
        Some(typed) => match typed.strip_prefix("~/") {
            Some(rest) => home.ok_or_else(no_home)?.join(rest),
            None => PathBuf::from(typed),
        },
    };
    if !path.is_absolute() {
        return Err(format!(
            "\"{}\" is not a kubeconfig purlis can name: give its absolute path",
            path.display()
        ));
    }
    Ok(path)
}

fn config_root() -> Result<PathBuf, String> {
    purlis_core::machine::config_root()
        .ok_or_else(|| "There is no config home on this machine to keep a pool in.".to_owned())
}

/// [`cluster_contexts`], with `kubectl`.
pub fn contexts_with(kubectl: &Kubectl, kubeconfig: &Path) -> Result<ClusterContexts, String> {
    let (contexts, current) = kubectl
        .clone()
        .with_kubeconfig(kubeconfig)
        .contexts()
        .map_err(|why| crate::devices::shown_safely(&why.to_string()))?;
    Ok(ClusterContexts {
        kubeconfig: kubeconfig.display().to_string(),
        contexts,
        current,
    })
}

/// **The contexts of the kubeconfig `kubeconfig` names** (#3292), as kubectl lists them.
#[tauri::command]
#[specta::specta]
pub async fn cluster_contexts(kubeconfig: Option<String>) -> Result<ClusterContexts, String> {
    crate::off_the_window("reading the kubeconfig's contexts", move || {
        let path = kubeconfig_of(kubeconfig.as_deref(), home().as_deref())?;
        contexts_with(&kubectl()?, &path)
    })
    .await
}

/// [`cluster_pool_key`], with `kubectl`.
pub fn key_with(
    kubectl: &Kubectl,
    kubeconfig: &Path,
    context: &str,
    pool: &str,
) -> Result<ClusterPoolKey, String> {
    let pointed = kubectl.clone().pointed_at(kubeconfig, context);
    let said =
        |why: purlis_core::clusterpool::Refused| crate::devices::shown_safely(&why.to_string());
    let key = pointed.pool_pin(pool.trim()).map_err(said)?.key;
    let namespace = pointed.namespace().map_err(said)?;
    Ok(ClusterPoolKey {
        key: key.spelled(),
        namespace,
    })
}

/// **Check its key**: the cluster operator key the cluster's Pool `pool` names, through
/// `context` of `kubeconfig`, for the person to check before the pool is added (#3292).
#[tauri::command]
#[specta::specta]
pub async fn cluster_pool_key(
    kubeconfig: Option<String>,
    context: String,
    pool: String,
) -> Result<ClusterPoolKey, String> {
    crate::off_the_window("reading the cluster operator's key", move || {
        let path = kubeconfig_of(kubeconfig.as_deref(), home().as_deref())?;
        key_with(&kubectl()?, &path, &context, &pool)
    })
    .await
}

/// **Adds a cluster pool on this machine** (#3292): pins the key the person checked, claims a
/// runner of the cluster's Pool, waits for its operator to vouch for it, pairs it and keeps it.
/// What fails is taken back.
#[tauri::command]
#[specta::specta]
pub async fn add_cluster_pool(adding: ClusterPoolAdding) -> Result<ClusterPoolRow, String> {
    crate::off_the_window("adding a cluster pool", move || {
        let config = config_root()?;
        add_on(
            &config,
            &kubectl()?,
            &adding,
            home().as_deref(),
            purlis_core::pools::VOUCHED_WITHIN,
        )
    })
    .await
}

/// [`add_cluster_pool`], on `config` with `kubectl`, waiting at most `within` for the operator.
pub fn add_on(
    config: &Path,
    kubectl: &Kubectl,
    adding: &ClusterPoolAdding,
    home: Option<&Path>,
    within: std::time::Duration,
) -> Result<ClusterPoolRow, String> {
    let kubeconfig = kubeconfig_of(adding.kubeconfig.as_deref(), home)?;
    let pointed = kubectl
        .clone()
        .pointed_at(&kubeconfig, adding.context.trim());
    let said =
        |why: purlis_core::clusterpool::Refused| crate::devices::shown_safely(&why.to_string());
    // The key and cluster the Pool names now: pinned only where the key is the one checked.
    let pin = pointed.pool_pin(adding.pool.trim()).map_err(said)?;
    if pin.key.spelled() != adding.key.trim() {
        return Err(format!(
            "The cluster's pool {} now names another cluster operator key than the one you \
             checked, so nothing was added. Check its key again.",
            adding.pool.trim()
        ));
    }
    let namespace = pointed.namespace().map_err(said)?;
    let pool = ClusterPool::new(
        adding.name.trim(),
        &kubeconfig,
        adding.context.trim(),
        adding.pool.trim(),
        &namespace,
        pin,
    )
    .map_err(|why| why.to_string())?;
    let desktop =
        purlis_core::linkkey::desktop(config, &*purlis_core::linkkey::this_builds_keyring(config))?
            .pinned();
    purlis_core::pools::add_cluster(config, kubectl, &pool, &desktop, within)
        .map_err(|why| crate::dockerpools::refused_said(&why.to_string()))?;
    Ok(row(&pool))
}

/// **Removes the cluster pool `name`** on `config` (#3292): this machine out of its runner's
/// peers over a link of its own while it answers, then its claim deleted, and with it its runner.
pub fn remove_on(config: &Path, name: &str) -> Result<Option<String>, String> {
    let kubectl = kubectl()?;
    let unpair = |pool: &ClusterPool, kept: &ClusterKept| {
        let to = ToARunner::of_cluster(config, &kubectl, pool, kept)?;
        crate::runnerlinks::unpair(&to)
    };
    purlis_core::pools::remove_cluster(config, &kubectl, name, Some(&unpair))
        .map_err(|why| crate::dockerpools::refused_said(&why.to_string()))?;
    crate::notheld::recheck(config);
    Ok(None)
}

/// **Where the cluster pool `name`'s runner stands** on `config` (#3292): its cluster operator
/// key changed, its runner's key changed, or one link made and let go of.
pub fn link_on(config: &Path, name: &str) -> Result<PoolLink, String> {
    let (pool, kept) = purlis_core::runners::cluster_runner_for(config, name)
        .map_err(|why| why.to_string())?
        .ok_or_else(|| format!("no cluster pool named {name} is on this machine"))?;
    let kubectl = kubectl()?;
    let to = ToARunner::of_cluster(config, &kubectl, &pool, &kept)?;
    link_through(config, &kubectl, name, || {
        crate::runnerlinks::link_state(&to)
    })
}

/// [`link_on`], with `kubectl` (found on this machine): the check every link makes first
/// ([`purlis_core::pools::before_cluster_link`]), said as the row draws it, then `link` where it
/// passes.
pub fn link_through(
    config: &Path,
    kubectl: &Kubectl,
    name: &str,
    link: impl FnOnce() -> Result<PoolLink, String>,
) -> Result<PoolLink, String> {
    use purlis_core::clusterpool::Refused;
    let now = crate::sandboxing::now_secs();
    match purlis_core::pools::before_cluster_link(config, kubectl, name, now) {
        Err(Refused::OperatorKeyChanged { .. }) => Ok(PoolLink::OperatorKeyChanged),
        Err(Refused::KeyChanged { .. }) => Ok(PoolLink::KeyChanged),
        Err(why) => Ok(PoolLink::Down {
            why: crate::devices::shown_safely(&why.to_string()),
        }),
        Ok(()) => link(),
    }
}

#[cfg(test)]
mod tests;
