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
    /// Whether the person marked it as their own (#4071).
    pub owned: bool,
}

/// `pool`, as its row draws it.
pub fn row(pool: &ClusterPool) -> ClusterPoolRow {
    ClusterPoolRow {
        name: pool.name().to_owned(),
        context: pool.context().to_owned(),
        pool: pool.pool().to_owned(),
        namespace: pool.namespace().to_owned(),
        operator_key: pool.operator().spelled(),
        owned: pool.owned(),
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
    /// The longest wait for its runner to be vouched for, in minutes, where the person raised it
    /// after an add that ran out (spec #4275): saved to the pool. None is purlis's default.
    pub vouch_wait: Option<u32>,
}

/// **This machine's `kubectl`, by its absolute path** (#3292,
/// [`Kubectl::on_this_machine`]): where its installers put it, else an absolute folder of the
/// window's `PATH`; its caches kept in a folder of purlis's under `config`
/// ([`purlis_core::runners::kubectl_cache`]).
pub fn kubectl(config: &Path) -> Result<Kubectl, String> {
    purlis_core::pools::kubectl_here(config)
}

/// **The context and the cluster's Pool, held to what purlis names to kubectl** (#3292), before
/// kubectl runs: a context that cannot read as a flag, and a Pool named as Kubernetes names it.
pub fn usable(context: &str, pool: &str) -> Result<(), String> {
    if !purlis_core::clusterpool::is_context(context.trim()) {
        return Err(format!(
            "\"{}\" is not a kubeconfig context purlis can name. Choose one of the kubeconfig's \
             contexts.",
            context.trim()
        ));
    }
    if !purlis_core::clusterpool::is_subdomain(pool.trim()) {
        return Err(format!(
            "\"{}\" is not a cluster pool's name. Write it as your cluster's admin gives it: \
             lower-case letters, digits, dashes and dots.",
            pool.trim()
        ));
    }
    Ok(())
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
        contexts_with(&kubectl(&config_root()?)?, &path)
    })
    .await
}

/// **A runner of the cluster pool `pool` for the chat `chat` alone** (#3818), made now or already
/// made, waiting for it for at most the pool's `vouchWait`. Answers the name a link reaches it by.
pub fn fresh_runner(config: &Path, pool: &str, chat: &str) -> Result<String, String> {
    let keyring = purlis_core::linkkey::this_builds_keyring(config);
    let desktop = purlis_core::linkkey::desktop(config, &*keyring)
        .map_err(|why| why.to_string())?
        .pinned();
    let within = purlis_core::runners::cluster_pools(config)
        .map_err(|why| why.to_string())?
        .iter()
        .find(|one| one.name() == pool)
        .map_or(purlis_core::pools::VOUCHED_WITHIN, ClusterPool::vouch_wait);
    purlis_core::pools::claim_fresh(config, &kubectl(config)?, pool, chat, &desktop, within)
        .map(|kept| kept.link_name())
        .map_err(|why| why.to_string())
}

/// **The chat `chat` ended, so its fresh runner goes** (#3818), on a thread of its own. A claim
/// that could not be deleted keeps its record, and is tried as the chat is closed again.
pub fn fresh_runner_ended(config: &Path, chat: &str) {
    if !matches!(
        purlis_core::runners::fresh_runner_of(config, chat),
        Ok(Some(_))
    ) {
        return;
    }
    let (config, chat) = (config.to_path_buf(), chat.to_owned());
    std::thread::spawn(move || {
        let released = kubectl(&config).and_then(|kubectl| {
            purlis_core::pools::release_fresh(&config, &kubectl, &chat)
                .map_err(|why| why.to_string())
        });
        if let Err(why) = released {
            tracing::warn!("purlis: a chat's fresh runner was not released: {why}");
        }
    });
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
        usable(&context, &pool)?;
        let path = kubeconfig_of(kubeconfig.as_deref(), home().as_deref())?;
        key_with(&kubectl(&config_root()?)?, &path, &context, &pool)
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
        usable(&adding.context, &adding.pool)?;
        let config = config_root()?;
        let within = vouch_wait_of(&adding)?;
        add_on(
            &config,
            &kubectl(&config)?,
            &adding,
            home().as_deref(),
            within,
        )
    })
    .await
}

/// How long adding this pool waits for the operator: the value it was raised to, checked as any
/// other, or purlis's default. Only the wait changes: the operator's vouching is asked for all
/// the same.
fn vouch_wait_of(adding: &ClusterPoolAdding) -> Result<std::time::Duration, String> {
    let limit = purlis_core::poollimits::LimitId::VouchWait.limit();
    let minutes = adding.vouch_wait.map_or(limit.default, u64::from);
    limit.check(minutes, None)?;
    limit
        .unit
        .as_time(minutes)
        .ok_or_else(|| format!("{} is a length of time", limit.plain))
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
    let pool = match adding.vouch_wait {
        Some(minutes) => pool.with_limit(
            purlis_core::poollimits::LimitId::VouchWait,
            u64::from(minutes),
        ),
        None => pool,
    };
    let desktop =
        purlis_core::linkkey::desktop(config, &*purlis_core::linkkey::this_builds_keyring(config))?
            .pinned();
    purlis_core::pools::add_cluster(config, kubectl, &pool, &desktop, within)
        .map_err(|why| add_refused(adding.name.trim(), within, &why))?;
    Ok(row(&pool))
}

/// What a refused add says: only one the wait itself ended offers to raise it (#4398), as this
/// machine's own loop found it, never from the cluster's words.
fn add_refused(
    name: &str,
    within: std::time::Duration,
    why: &purlis_core::runners::Refused,
) -> String {
    let said = crate::dockerpools::refused_said(&why.to_string());
    if matches!(why, purlis_core::runners::Refused::WaitRanOut(_)) {
        ran_out_vouching(name, within, &said)
    } else {
        said
    }
}

/// An add that ran out of the wait `within`, in words the window can ask of for what to raise it
/// to (spec #4275).
fn ran_out_vouching(name: &str, within: std::time::Duration, said: &str) -> String {
    let limit = purlis_core::poollimits::LimitId::VouchWait.limit();
    let minutes = within.as_secs().div_ceil(60);
    let hit = limit.waited(minutes, None);
    format!("{said} {}", purlis_core::poollimits::hit_on(name, hit))
}

/// **The person marks the cluster pool `name` as their own, or takes the mark back** (#4071, the
/// person's ruling of 2026-10-30). The window's alone (`ui::WINDOW_ONLY`): no chat, Pool or
/// operator reaches it. Taken back, every login opted in on the pool is forgotten, so a later
/// mark asks about each again.
#[tauri::command]
#[specta::specta]
pub async fn set_cluster_pool_owned(
    planes: tauri::State<'_, crate::planes::Planes>,
    name: String,
    owned: bool,
) -> Result<(), String> {
    let held = planes.every_held();
    crate::off_the_window("marking a cluster pool", move || {
        purlis_core::runners::set_cluster_owned(&config_root()?, &name, owned)
            .map_err(|why| why.to_string())?;
        if !owned {
            for project in held {
                project.browsers().forget_pool_logins(&name);
            }
            crate::poolprofiles::cancel_sign_ins_on(&name);
        }
        Ok(())
    })
    .await
}

/// The event a pool's beginning to mint without asking raises its Notice on (#4442).
pub const POOL_MINTS_WITHOUT_ASKING: &str = "pool-mints-without-asking";

/// [`pools_minting_without_asking`], on `config`.
fn minting_without_asking(config: &Path) -> Result<Vec<String>, String> {
    Ok(purlis_core::runners::cluster_pools(config)
        .map_err(|why| why.to_string())?
        .iter()
        .filter(|pool| pool.mint_notice_due())
        .map(|pool| pool.name().to_owned())
        .collect())
}

/// **The cluster pools whose Pool began minting without asking, for the person to be told** (#4442,
/// the person's ruling of 2026-11-01): one Notice for each change. It holds and refuses nothing, as
/// the admin's opt-in stands. The window's alone (`ui::WINDOW_ONLY`).
#[tauri::command]
#[specta::specta]
pub async fn pools_minting_without_asking() -> Result<Vec<String>, String> {
    crate::off_the_window("reading the pools that mint without asking", move || {
        minting_without_asking(&config_root()?)
    })
    .await
}

/// **The person put away the Notice of [`pools_minting_without_asking`] for pool `name`** (#4442).
/// The window's alone (`ui::WINDOW_ONLY`): a chat that could press it could hide the Notice.
#[tauri::command]
#[specta::specta]
pub async fn put_away_pool_minting_notice(
    app: tauri::AppHandle,
    name: String,
) -> Result<(), String> {
    crate::off_the_window("putting away a pool's notice", move || {
        purlis_core::runners::put_away_mint_notice(&config_root()?, &name)
            .map_err(|why| why.to_string())
    })
    .await?;
    // Every window reads it again, so none keeps what was put away.
    let _ = tauri::Emitter::emit(&app, POOL_MINTS_WITHOUT_ASKING, ());
    Ok(())
}

/// **Removes the cluster pool `name`** on `config` (#3292): this machine out of its runner's
/// peers over a link of its own while it answers, then its claim deleted, and with it its runner.
///
/// The pool is forgotten here whatever the cluster answers, and what may be left there is said,
/// with how to delete it: a missing kubectl, a cluster that does not answer or a claim that is
/// not this desktop's never keeps it on this machine.
pub fn remove_on(config: &Path, name: &str) -> Result<Option<String>, String> {
    let kubectl = kubectl(config).ok();
    let device =
        purlis_core::linkkey::kept(config, &*purlis_core::linkkey::this_builds_keyring(config))
            .ok()
            .map(|desktop| desktop.pinned().device);
    let unpair = |pool: &ClusterPool, kept: &ClusterKept| {
        let kubectl = kubectl
            .as_ref()
            .ok_or_else(|| "purlis found no kubectl on this machine".to_owned())?;
        let to = ToARunner::of_cluster(config, kubectl, pool, kept)?;
        crate::runnerlinks::unpair(&to)
    };
    let left = purlis_core::pools::remove_cluster(
        config,
        kubectl.as_ref(),
        name,
        device.as_deref(),
        Some(&unpair),
    )
    .map_err(|why| crate::dockerpools::refused_said(&why.to_string()))?;
    crate::notheld::recheck(config);
    Ok(crate::dockerpools::left_said(left))
}

/// **Where the cluster pool `name`'s runner stands** on `config` (#3292): its cluster operator
/// key changed, its runner's key changed, or one link made and let go of.
pub fn link_on(config: &Path, name: &str) -> Result<PoolLink, String> {
    let (pool, kept) = purlis_core::runners::cluster_runner_for(config, name)
        .map_err(|why| why.to_string())?
        .ok_or_else(|| format!("no cluster pool named {name} is on this machine"))?;
    let kubectl = kubectl(config)?;
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
pub(crate) mod tests;
