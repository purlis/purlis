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
    /// The cluster operator key the Pool's status names, spelled as it gives it: only once it is
    /// confirmed (#4495), by the key pasted from the admin, or as the one this setup made.
    pub key: Option<String>,
    /// Whether this machine made that key in this Pool setup, so no paste is asked for (#4495).
    pub made_here: bool,
    /// Whether the key pasted is the Pool's, where one was pasted (#4495).
    pub matches: Option<bool>,
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
    /// The cluster operator key the person pasted, as their cluster's admin gave it (#4495).
    /// None only where this machine made the signing key in this Pool setup.
    pub pasted: Option<String>,
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

/// Where a chat's runner has got to, as the window waiting on it says it (#3709).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RunnerStage {
    /// The stage, in the words the person reads.
    pub said: String,
    /// One line more about it.
    pub more: String,
    /// The cluster's own words for why it waits, where it gave them.
    pub cluster_said: Option<String>,
}

/// **Where the runner a chat in `workspace` of the project at `root` starts on has got to**
/// (#3709), where its `start` chose where it runs or its levels say (#4536): nothing where it
/// runs here, on a Docker pool, or on a runner not on its way.
pub fn stage_of(
    root: &Path,
    workspace: Option<&str>,
    start: Option<&purlis_core::machine::StartChoice>,
) -> Result<Option<RunnerStage>, String> {
    let config = config_root()?;
    let Some(pool) = purlis_core::pools::chat_place(&config, root, workspace, start)?.pool else {
        return Ok(None);
    };
    let Ok(Some((cluster, kept))) = purlis_core::runners::cluster_runner_for(&config, &pool) else {
        return Ok(None);
    };
    // Held to a time: the window asks again after each answer.
    let kubectl =
        cluster.kubectl(&kubectl(&config)?.within(purlis_core::pools::VOUCHED_NOW_WITHIN));
    let stage = kubectl.claim_stage(&kept.claimed.namespace, &kept.claimed.name);
    Ok(stage.map(|stage| RunnerStage {
        said: stage.said.to_owned(),
        more: stage.more.to_owned(),
        cluster_said: stage
            .cluster_said
            .map(|said| crate::devices::shown_safely(&said)),
    }))
}

/// **Where the runner of a chat being started has got to** (#3709), read from the cluster, for the
/// window's Start while it waits. Nothing where there is no stage to say.
#[tauri::command]
#[specta::specta]
pub async fn runner_stage(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
    runs_on: Option<crate::RunsOnPick>,
) -> Result<Option<RunnerStage>, String> {
    let held = planes.held(&plane)?;
    let start = runs_on.map(crate::RunsOnPick::choice).transpose()?;
    crate::off_the_window("reading where the runner has got to", move || {
        stage_of(held.root(), workspace.as_deref(), start.as_ref())
    })
    .await
}

/// **A runner of the cluster pool `pool` for the chat `chat` alone** (#3818), made now or already
/// made, waiting for it for at most the pool's `vouchWait`, its work going back to `of` (#4569).
/// Answers the name a link reaches it by.
pub fn fresh_runner(
    config: &Path,
    pool: &str,
    chat: &str,
    of: purlis_core::runners::FreshOf,
) -> Result<String, String> {
    let keyring = purlis_core::linkkey::this_builds_keyring(config);
    let desktop = purlis_core::linkkey::desktop(config, &*keyring)
        .map_err(|why| why.to_string())?
        .pinned();
    let within = purlis_core::runners::cluster_pools(config)
        .map_err(|why| why.to_string())?
        .iter()
        .find(|one| one.name() == pool)
        .map_or(purlis_core::pools::VOUCHED_WITHIN, ClusterPool::vouch_wait);
    purlis_core::pools::claim_fresh(config, &kubectl(config)?, pool, chat, of, &desktop, within)
        .map(|kept| kept.link_name())
        .map_err(|why| why.to_string())
}

/// **The fresh Claims of this desktop that a crash left with no record go** (#4569), off the
/// window: [`purlis_core::pools::sweep_orphan_fresh_claims`].
pub fn sweep_orphan_claims(config: &Path) {
    let swept = kubectl(config).and_then(|kubectl| {
        let keyring = purlis_core::linkkey::this_builds_keyring(config);
        let desktop = purlis_core::linkkey::desktop(config, &*keyring)
            .map_err(|why| why.to_string())?
            .pinned();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        purlis_core::pools::sweep_orphan_fresh_claims(config, &kubectl, &desktop.device, now)
            .map_err(|why| why.to_string())
    });
    match swept {
        Ok(deleted) if !deleted.is_empty() => tracing::info!(
            "purlis: deleted fresh claims no record held, left by a crash: {}",
            deleted.join(", ")
        ),
        Ok(_) => {}
        Err(why) => tracing::info!("purlis: claims no record holds were not swept: {why}"),
    }
}

/// How a chat's fresh runner's end came out (#4569).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FreshEnd {
    /// It went: its work was proved here, or the person discarded it.
    Went,
    /// It stays, with its record, and `why` says what is not here. `gone` where its cluster says
    /// its runner is gone already, with what it held: the record stays until the person forgets
    /// it, so that is never silent.
    Kept { why: Vec<String>, gone: bool },
}

/// **The chat `chat` ended, and `missing` says what of its fresh runner's work is not here**
/// (#3818, #4569): none only on proof, or where nothing is asked for (a start that failed, or the
/// person's Discard). Only then does the runner go. Otherwise its claim stays with its record,
/// for Fetch again, the sweep or Discard, and the window says so.
///
/// `proved` where `missing` is a proof's: then the operator's word is read again, since a proof
/// is trusted only while that word holds. Where it no longer does, the runner may have been
/// suspended and started again with an empty home, which would prove nothing.
pub fn fresh_runner_ended(config: &Path, chat: &str, missing: &[String], proved: bool) -> FreshEnd {
    let kubectl = match kubectl(config) {
        Ok(kubectl) => kubectl,
        Err(why) => {
            return FreshEnd::Kept {
                why: missing
                    .iter()
                    .cloned()
                    .chain([format!("purlis could not reach its cluster: {why}")])
                    .collect(),
                gone: false,
            };
        }
    };
    if !missing.is_empty() {
        // Never given up on the cluster's word alone: the person forgets it.
        let gone = matches!(
            purlis_core::pools::fresh_runner_lost(config, &kubectl, chat),
            Ok(true)
        );
        tracing::warn!(
            "purlis: a chat's fresh runner stays, since its work is not all here: {}",
            missing.join(" ")
        );
        return FreshEnd::Kept {
            why: missing.to_vec(),
            gone,
        };
    }
    if proved {
        let pool = purlis_core::runners::fresh_runner_of(config, chat)
            .ok()
            .flatten()
            .map(|(pool, _)| pool);
        let word = pool.map_or_else(
            || Err("its pool is not on this machine".to_owned()),
            |pool| operators_word(config, &pool),
        );
        if let Some(kept) = kept_without_the_word(word) {
            return kept;
        }
    }
    match purlis_core::pools::release_fresh(config, &kubectl, chat) {
        Ok(()) => FreshEnd::Went,
        Err(why) => {
            tracing::warn!("purlis: a chat's fresh runner stays for now: {why}");
            FreshEnd::Kept {
                why: vec![release_failed_said(proved, &why.to_string())],
                gone: false,
            }
        }
    }
}

/// What a release that failed says (#5393): that all the runner's work is here only where a proof
/// said so, and not for a runner the person discarded, whatever it holds.
fn release_failed_said(proved: bool, why: &str) -> String {
    if proved {
        format!("Its work is all here, but its claim could not be deleted: {why}")
    } else {
        format!("It was to go, but its claim could not be deleted: {why}")
    }
}

/// **A proof is trusted only while the operator's word holds** (D-4569-18): where it no longer
/// does, the runner is kept, with why.
fn kept_without_the_word(word: Result<(), String>) -> Option<FreshEnd> {
    word.err().map(|why| FreshEnd::Kept {
        why: vec![format!(
            "Its work looks all here, but purlis trusts that only while its cluster operator says \
             it keeps such a runner through idle, and now {why}."
        )],
        gone: false,
    })
}

/// **Why a short task on the cluster pool `pool` stays on the pool's own runner**, where it does
/// (#4569): only an operator whose signed word, holding now, says it keeps a fresh Claim's runner
/// through idle holds a task's commits while its desktop is away. Not knowing is a no.
pub fn why_no_fresh_runner(config: &Path, pool: &ClusterPool) -> Option<String> {
    no_fresh_runner_said(pool.name(), operators_word(config, pool))
}

/// **Whether the operator of `pool` says now that a fresh runner stays** (#4569), read each time
/// it counts: at a start, and again before a fresh runner is let go of on proof.
pub fn operators_word(config: &Path, pool: &ClusterPool) -> Result<(), String> {
    let kubectl = kubectl(config)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    purlis_core::pools::fresh_runners_stay(pool, &kubectl, now).map_err(|why| why.to_string())
}

/// **Why a short task of the project at `root` stays on pool `pool`'s own runner, where its folder
/// is no git repository** (D-4569-21): only a repository's files can be proved home, so what a
/// task wrote outside its clones there could never be, and its runner would go with it.
pub fn no_fresh_runner_for_the_project(root: &Path, pool: &str) -> Option<String> {
    (!purlis_core::codelink::a_repository(root)).then(|| {
        format!(
            "This task runs on pool {pool}'s own runner, since this project's folder is not a git \
             repository, so purlis could not check that a runner of its own holds nothing the \
             project lacks before letting it go."
        )
    })
}

/// [`why_no_fresh_runner`], of what its operator answered.
fn no_fresh_runner_said(pool: &str, stays: Result<(), String>) -> Option<String> {
    stays.err().map(|why| {
        format!(
            "This task runs on pool {pool}'s own runner, since {why}, so a runner of its own \
             might not keep the task's work. Update the operator, or check that it runs, to \
             change that."
        )
    })
}

/// **A fresh runner of the pool `name` that still holds a chat's work**, its chat's id, where
/// there is one: a pool's removal would delete it with no fetch (#4569). A cluster that does not
/// answer holds up nothing here, and [`purlis_core::pools::remove_cluster`] leaves its Claim.
fn a_fresh_runner_at_work(config: &Path, kubectl: &Kubectl, name: &str) -> Option<String> {
    purlis_core::runners::fresh_runners(config)
        .unwrap_or_default()
        .into_iter()
        .filter(|kept| kept.name == name)
        .filter_map(|kept| kept.chat)
        .find(|chat| {
            matches!(
                purlis_core::pools::fresh_runner_lost(config, kubectl, chat),
                Ok(false)
            )
        })
}

/// [`cluster_pool_key`], with `kubectl`.
pub fn key_with(
    config: &Path,
    kubectl: &Kubectl,
    kubeconfig: &Path,
    context: &str,
    pool: &str,
    pasted: Option<&str>,
) -> Result<ClusterPoolKey, String> {
    let pointed = kubectl.clone().pointed_at(kubeconfig, context);
    let said =
        |why: purlis_core::clusterpool::Refused| crate::devices::shown_safely(&why.to_string());
    let pin = pointed.pool_pin(pool.trim()).map_err(said)?;
    let namespace = pointed.namespace().map_err(said)?;
    Ok(key_shown(config, context, &pin, pasted, namespace))
}

/// **What Check its key says of the key the Pool names (`pin`, with its cluster)** (#4495): the
/// key itself only once it is confirmed, by `pasted` or as the one this setup made on `context`
/// in that cluster, so the box for the admin's key is never filled from the cluster's own word.
fn key_shown(
    config: &Path,
    context: &str,
    pin: &purlis_core::clusterpool::PoolPin,
    pasted: Option<&str>,
    namespace: String,
) -> ClusterPoolKey {
    let key = &pin.key;
    let made_here = purlis_core::poolsetup::made_here(config, context.trim(), pin);
    let matches = pasted
        .filter(|pasted| !pasted.trim().is_empty())
        .map(|pasted| {
            purlis_core::clusterpool::OperatorKey::from_pasted(pasted).is_ok_and(|one| &one == key)
        });
    ClusterPoolKey {
        key: (made_here || matches == Some(true)).then(|| key.spelled()),
        made_here,
        matches,
        namespace,
    }
}

/// **Check its key**: the cluster operator key the cluster's Pool `pool` names, through
/// `context` of `kubeconfig`, for the person to check before the pool is added (#3292).
#[tauri::command]
#[specta::specta]
pub async fn cluster_pool_key(
    kubeconfig: Option<String>,
    context: String,
    pool: String,
    pasted: Option<String>,
) -> Result<ClusterPoolKey, String> {
    crate::off_the_window("reading the cluster operator's key", move || {
        usable(&context, &pool)?;
        let path = kubeconfig_of(kubeconfig.as_deref(), home().as_deref())?;
        let config = config_root()?;
        key_with(
            &config,
            &kubectl(&config)?,
            &path,
            &context,
            &pool,
            pasted.as_deref(),
        )
    })
    .await
}

/// **Adds a cluster pool on this machine** (#3292): pins the key the person checked, claims a
/// runner of the cluster's Pool, waits for its operator to vouch for it, pairs it and keeps it.
/// What fails is taken back.
#[tauri::command]
#[specta::specta]
pub async fn add_cluster_pool(
    app: tauri::AppHandle,
    adding: ClusterPoolAdding,
) -> Result<ClusterPoolRow, crate::poolprofiles::LimitRefused> {
    let added = crate::off_the_window("adding a cluster pool", move || {
        Ok(crate::poolprofiles::offering_limits(|| {
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
        }))
    })
    .await?;
    // Every window asks again which chats run on an older sandbox: one started before the add
    // holds none of the programs the pool runs until it restarts (#1750).
    if added.is_ok() {
        let _ = tauri::Emitter::emit(&app, CLUSTER_POOL_ADDED, ());
    }
    added
}

/// The event an added cluster pool raises, on which each window asks again which chats run on
/// an older sandbox (#1750).
pub const CLUSTER_POOL_ADDED: &str = "cluster-pool-added";

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
    // The key and cluster the Pool names now: pinned only where the key is the one pasted from
    // the cluster's admin, however it was copied, or the one this setup made (#4495).
    let pin = pointed.pool_pin(adding.pool.trim()).map_err(said)?;
    let confirmed = purlis_core::poolsetup::confirm_key(
        config,
        adding.context.trim(),
        &pin,
        adding.pasted.as_deref(),
    )
    .map_err(|why| crate::devices::shown_safely(&why.to_string()))?;
    let namespace = pointed.namespace().map_err(said)?;
    let pool = ClusterPool::new(
        adding.name.trim(),
        &kubeconfig,
        adding.context.trim(),
        adding.pool.trim(),
        &namespace,
        pin,
    )
    .map_err(|why| why.to_string())?
    .confirmed_by(confirmed);
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

/// An add that ran out of the wait `within` (spec #4275).
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
    if let Some(at_work) = kubectl
        .as_ref()
        .and_then(|kubectl| a_fresh_runner_at_work(config, kubectl, name))
    {
        tracing::info!("purlis: chat {at_work} keeps pool {name} from being removed");
        return Err(format!(
            "A task on pool {name} has a runner of its own, and its work has not come back. \
             Close the task, or open its project so purlis can fetch it, then remove the pool."
        ));
    }
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
