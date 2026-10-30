// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How this desktop links to a pool's runner** (ADR 0078 §2 and §3, #2679): the runner kept
//! under the pool's name in `runners.json`, reached through the container preset
//! (`docker exec -i --user <uid>:<gid> <container> purlis bridge`), the container checked as it
//! was made before every connection, and linked with this desktop's link key against the key
//! pinned at the runner's pairing. Nothing here takes a name that is not kept
//! ([`purlis_core::runners::runner_for`]).
//!
//! The session host the window starts relays a window's link through it (`relay_to`,
//! [`relays_to`]).
//!
//! **A cluster pool's runner is reached through this machine's `kubectl`** (#3292,
//! [`ToARunner::of_cluster`]): `kubectl exec -i` into the pod its pairing named, pointed at the
//! pool's kubeconfig and context and handed no `KUBECONFIG` of this process's, after a check
//! before every connection that the cluster's Pool still names the operator key pinned as the
//! pool was added and that the operator still vouches for the pod and key the runner was paired
//! with ([`purlis_core::clusterpool::before_link`]). Either refused is told as a changed key, so
//! the window asks the person about it as it does for a Docker runner's.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use purlis_core::clusterpool::Kubectl;
use purlis_core::dockerpool::Docker;
use purlis_core::runners::{ClusterKept, ClusterPool, Kept};
use purlis_host::{CommandConnector, NotChecked};
use purlis_session_protocol::hostlink::{self, Handshake, Identity, LinkKey, Liveness, Pinned};

/// What a link to one kept runner needs: its connector, this desktop's device id and key, and
/// the runner as it was pinned.
#[derive(Clone)]
pub struct ToARunner {
    connector: CommandConnector,
    device: String,
    key: LinkKey,
    pinned: Pinned,
}

/// Where this machine's runners are: its config home, its `docker` and the keyring its link
/// key is in.
#[derive(Clone)]
pub struct Machine {
    pub config: PathBuf,
    pub docker: Docker,
    pub keyring: Arc<dyn purlis_core::secrets::keyring::Store>,
}

impl Machine {
    /// This machine, as the window and its host find it: `config`, its `docker`, and this
    /// build's keyring.
    pub fn here(config: &Path) -> Result<Machine, String> {
        Ok(Machine {
            config: config.to_path_buf(),
            docker: docker()?,
            keyring: Arc::from(purlis_core::linkkey::this_builds_keyring(config)),
        })
    }

    /// This desktop on a link, its key as it is kept: never made here, so a key the keyring
    /// does not give is said rather than replaced with one no runner pinned.
    pub fn desktop(&self) -> Result<purlis_core::linkkey::Desktop, String> {
        purlis_core::linkkey::kept(&self.config, &*self.keyring)
    }

    /// **Whether the runner `kept`'s container runs**, as Docker says, writing when it ran as
    /// it is read (#3492, [`purlis_core::runnerhours::seen`]). What could not be written is
    /// said, and the answer stands.
    pub fn running(&self, kept: &Kept) -> Result<bool, purlis_core::dockerpool::Refused> {
        let ran = self.docker.ran(&kept.made)?;
        hours_seen(&self.config, kept, ran);
        Ok(ran.running)
    }

    /// This desktop as a pool is added: its key made where it keeps none.
    pub fn desktop_made(&self) -> Result<purlis_core::linkkey::Desktop, String> {
        purlis_core::linkkey::desktop(&self.config, &*self.keyring)
    }
}

/// Writes when `kept` ran, as Docker's `ran` says (#3492); what could not be written is said.
fn hours_seen(config: &Path, kept: &Kept, ran: purlis_core::dockerpool::Ran) {
    let now = crate::sandboxing::now_secs();
    if let Err(why) = purlis_core::runnerhours::seen(config, kept.pool(), kept.name(), ran, now) {
        tracing::warn!("purlis: {why}");
    }
}

impl ToARunner {
    /// The runner kept for `pool` on `machine`: only one this machine paired and keeps, and
    /// never one draining (#2749).
    #[cfg(test)]
    pub fn of(machine: &Machine, pool: &str) -> Result<ToARunner, String> {
        let kept = purlis_core::runners::runner_for(&machine.config, pool)
            .map_err(|why| why.to_string())?;
        ToARunner::of_kept(machine, &kept)
    }

    /// The runner `kept`, as `machine` keeps it: a draining one too, which keeps its chats
    /// until each ends (#2749).
    ///
    /// **A suspended runner is left as it is** (#2051): a check, a poll, an unpairing or a stop
    /// never wakes one; only the window's relayed link does ([`Self::of_kept_waking`]).
    pub fn of_kept(machine: &Machine, kept: &Kept) -> Result<ToARunner, String> {
        let desktop = machine.desktop()?;
        Ok(ToARunner::kept(
            kept,
            machine,
            desktop.into_identity(),
            false,
        ))
    }

    /// [`Self::of_kept`], **a suspended runner started again** as it is linked to (#2051): one a
    /// day without a link stopped its own container, which is checked as it was made before it
    /// starts. The window's relayed link alone: its chats are what the person asked for.
    pub fn of_kept_waking(machine: &Machine, kept: &Kept) -> Result<ToARunner, String> {
        let desktop = machine.desktop()?;
        Ok(ToARunner::kept(
            kept,
            machine,
            desktop.into_identity(),
            true,
        ))
    }

    fn kept(kept: &Kept, machine: &Machine, desktop: Identity, resumes: bool) -> ToARunner {
        let docker = machine.docker.clone();
        let made = kept.made.clone();
        let checking = docker.clone();
        let preparing = docker.clone();
        let (config, of) = (machine.config.clone(), kept.clone());
        let connector = CommandConnector::new(
            docker.program().to_path_buf(),
            made.connect_args(),
            // The container is checked as it was made before every connection (ADR 0078 as
            // amended 2026-10-17, item 2): anything else refuses the link and runs nothing.
            Arc::new(move || {
                if resumes {
                    // When it ran, as read before it was started, then its start (#3492).
                    let resumed = checking.resumed(&made).map_err(|why| why.0)?;
                    hours_seen(&config, &of, resumed.before);
                    if resumed.started
                        && let Ok(ran) = checking.ran(&made)
                    {
                        hours_seen(&config, &of, ran);
                    }
                    Ok(())
                } else {
                    checking.check(&made).map_err(|why| why.0)
                }
            }),
        )
        // Each run of `docker exec` is given what every run of `docker` is: a checked copy of
        // Docker's settings and the engine pinned (#2752).
        .preparing(Arc::new(move || {
            let (env, cleared, held) = preparing.prepared().map_err(|why| why.0)?.into_parts();
            Ok(purlis_host::Prepared { env, cleared, held })
        }));
        ToARunner {
            connector,
            device: desktop.device,
            key: desktop.key,
            pinned: kept.pinned.clone(),
        }
    }

    /// **The cluster pool `pool`'s runner `kept`**, as this machine keeps it under `config`,
    /// reached through `kubectl` (#3292). Linking to it never starts anything: a claim's runner
    /// is the cluster's to run.
    pub fn of_cluster(
        config: &Path,
        kubectl: &Kubectl,
        pool: &ClusterPool,
        kept: &ClusterKept,
    ) -> Result<ToARunner, String> {
        let keyring = purlis_core::linkkey::this_builds_keyring(config);
        let desktop = purlis_core::linkkey::kept(config, &*keyring)?.into_identity();
        let checking = (
            config.to_path_buf(),
            kubectl.clone(),
            pool.name().to_owned(),
        );
        let kubectl = pool.kubectl(kubectl);
        let preparing = kubectl.clone();
        let connector = CommandConnector::checked(
            kubectl.program().to_path_buf(),
            // In its requester's own runner namespace, which every statement it is linked on
            // names (#2055).
            kubectl.connect_args(
                &purlis_core::clusterpool::runner_namespace(&kept.claimed.requester),
                &kept.pod,
            ),
            // Before every connection: the pin as it is kept now, followed through a roll and
            // kept again, and the runner vouched for (#3305).
            Arc::new(move || {
                let (config, kubectl, name) = &checking;
                let now = crate::sandboxing::now_secs();
                purlis_core::pools::before_cluster_link(config, kubectl, name, now)
                    .map_err(not_checked)
            }),
        )
        // Each run of `kubectl exec` is given what every run of this kubectl is (#3292).
        .preparing(Arc::new(move || {
            let (env, cleared) = preparing.environment();
            Ok(purlis_host::Prepared {
                env,
                cleared,
                held: Box::new(()),
            })
        }));
        Ok(ToARunner {
            connector,
            device: desktop.device,
            key: desktop.key,
            pinned: kept.pinned.clone(),
        })
    }

    /// A link kept to the runner for as long as what it answers is held
    /// ([`hostlink::keep_linked`]).
    pub fn keep(&self) -> hostlink::Links {
        hostlink::keep_linked(
            self.connector.clone(),
            Handshake::THIS_BUILD,
            Identity {
                device: self.device.clone(),
                key: self.key.clone(),
            },
            self.pinned.clone(),
            Liveness::default(),
        )
    }
}

/// **A cluster runner's check, said to the link** (#3292): a changed runner key or a changed
/// cluster operator key is a changed key, which the window asks the person about.
pub fn not_checked(why: purlis_core::clusterpool::Refused) -> NotChecked {
    use purlis_core::clusterpool::Refused;
    match why {
        Refused::KeyChanged { .. } | Refused::OperatorKeyChanged { .. } => {
            NotChecked::KeyChanged(why.to_string())
        }
        Refused::NotVouched(_) | Refused::Other(_) => NotChecked::Refused(why.to_string()),
    }
}

/// **The cluster runner a link that names `name` reaches** (#3292): the runner of the cluster
/// pool `name`, or `<pool>/<runner>` naming that same one. `None` where no cluster pool is
/// called so, for a Docker pool's name to be looked up instead.
pub fn cluster_reached(
    config: &Path,
    name: &str,
) -> Result<Option<(ClusterPool, ClusterKept)>, String> {
    let (pool, runner) = name.split_once('/').unwrap_or((name, name));
    match purlis_core::runners::cluster_runner_for(config, pool).map_err(|why| why.to_string())? {
        Some((pool, kept)) if kept.name == runner => Ok(Some((pool, kept))),
        Some(_) => Err(format!(
            "cluster pool {pool} has no runner {runner} on this machine, so nothing was reached"
        )),
        None => Ok(None),
    }
}

/// This machine's `docker`, where the window's `PATH` or Docker's installers put it.
pub fn docker() -> Result<Docker, String> {
    let path = purlis_core::envvar::var("PATH");
    let home = std::env::var_os("HOME").map(PathBuf::from);
    Docker::on_this_machine(path.as_deref(), home.as_deref()).map_err(|why| why.0)
}

/// **What the session host the window starts relays a window's link with** (`relay_to`, 1.33):
/// a link kept to the runner of the pool the window names, read from `config_root` as each
/// `relay_to` is taken, so a pool added or removed since is as it is now.
pub fn relays_to(config_root: PathBuf) -> purlis_host::RelaysTo {
    let docker = {
        let config_root = config_root.clone();
        relaying(move || Machine::here(&config_root))
    };
    // A cluster pool's runner, through kubectl (#3292); a Docker pool's otherwise.
    Arc::new(move |pool: &str| {
        let Some((cluster, kept)) = cluster_reached(&config_root, pool)? else {
            return docker(pool);
        };
        let to = ToARunner::of_cluster(
            &config_root,
            &crate::clusterpools::kubectl(&config_root)?,
            &cluster,
            &kept,
        )?;
        let resolved = kept.reached();
        Ok((Arc::new(move || to.keep()) as purlis_host::Keep, resolved))
    })
}

/// [`relays_to`], on the machine `machine` answers as each `relay_to` is taken.
pub fn relaying(
    machine: impl Fn() -> Result<Machine, String> + Send + Sync + 'static,
) -> purlis_host::RelaysTo {
    Arc::new(move |pool: &str| {
        let machine = machine()?;
        // A pool's runner, or one of its runners that drains (`<pool>/<runner>`, #2865).
        let (kept, _) = purlis_core::runners::reachable(&machine.config, pool)
            .map_err(|why| why.to_string())?;
        // A runner of another version than this desktop's is said, every time (#2749).
        if let Some(behind) = behind(&machine, &kept) {
            tracing::warn!("purlis: pool {pool}: {behind}");
        }
        let resolved = format!("{}/{}", kept.pool(), kept.name());
        let to = ToARunner::of_kept_waking(&machine, &kept)?;
        Ok((Arc::new(move || to.keep()) as purlis_host::Keep, resolved))
    })
}

/// **Every runner this machine keeps, as the session host's kill switch reaches them** (#2051,
/// ADR 0078 §7): each `<pool>/<runner>` kept under `config_root`, a draining one among them, with
/// a link kept to it as a relayed window's is, or why there is none. Read as each stop is sent.
pub fn every_runner(config_root: PathBuf) -> purlis_host::EveryRunner {
    Arc::new(move || {
        let kept = match purlis_core::runners::read(&config_root) {
            Ok(kept) => kept,
            Err(why) => {
                tracing::warn!("purlis: the runners this machine keeps could not be read: {why}");
                return Vec::new();
            }
        };
        let machine = Machine::here(&config_root);
        // A cluster pool's runner, through kubectl (#3292): nothing of it is suspended here.
        let clusters = cluster_runners_to_stop(&config_root);
        kept.iter()
            .filter_map(|kept| {
                let name = format!("{}/{}", kept.pool(), kept.name());
                let keep = match &machine {
                    Err(why) => Err(why.clone()),
                    Ok(machine) => match machine.running(kept) {
                        // Suspended: nothing runs there, and it is not woken to be stopped. A
                        // link made to it later carries the stop first while this machine is
                        // stopped.
                        Ok(false) => return None,
                        Ok(true) => ToARunner::of_kept(machine, kept),
                        Err(why) => Err(why.0),
                    },
                }
                .map(|to| Arc::new(move || to.keep()) as purlis_host::Keep);
                Some((name, keep))
            })
            .chain(clusters)
            .collect()
    })
}

/// Every cluster pool's runner kept under `config_root`, as the kill switch reaches it (#3292).
fn cluster_runners_to_stop(config_root: &Path) -> Vec<(String, Result<purlis_host::Keep, String>)> {
    let pools = purlis_core::runners::cluster_pools(config_root).unwrap_or_default();
    let kept = purlis_core::runners::cluster_runners(config_root).unwrap_or_default();
    let kubectl = crate::clusterpools::kubectl(config_root);
    kept.iter()
        .filter_map(|kept| {
            let pool = pools.iter().find(|pool| pool.name() == kept.name)?;
            let keep = kubectl
                .clone()
                .and_then(|kubectl| ToARunner::of_cluster(config_root, &kubectl, pool, kept))
                .map(|to| Arc::new(move || to.keep()) as purlis_host::Keep);
            Some((format!("{}/{}", pool.name(), kept.name), keep))
        })
        .collect()
}

/// **Which runner the session host the window starts relays to drains** (#2865): one reached as
/// `<pool>/<runner>`, which keeps its chats and starts no new one. Read from `config_root` as
/// each `relay_to` is taken.
pub fn drains_to(config_root: PathBuf) -> purlis_host::DrainsTo {
    Arc::new(move |name: &str| drains(&config_root, name))
}

/// **Whether the runner `<pool>/<runner>` kept under `config_root` takes no new chat** (#2865):
/// only a runner of that pool kept here and not draining takes one. A file that cannot be read,
/// a runner not kept, a name of another shape and one that drains all refuse it.
fn drains(config_root: &Path, name: &str) -> bool {
    let Some((pool, runner)) = name.split_once('/') else {
        return true;
    };
    // A cluster pool's one runner never drains (#3292).
    if let Ok(clusters) = purlis_core::runners::cluster_runners(config_root)
        && clusters
            .iter()
            .any(|kept| kept.name == pool && kept.name == runner)
    {
        return false;
    }
    match purlis_core::runners::read(config_root) {
        Ok(kept) => !kept
            .iter()
            .any(|kept| kept.pool() == pool && kept.name() == runner && !kept.draining),
        Err(_) => true,
    }
}

/// **What this desktop's version is**, as its own `purlis --version` would say it: the
/// version its release stamped it with (#2749).
pub fn desktop_version() -> &'static str {
    purlis_core::adopt::release_version()
}

/// **How `kept`'s version stands against this desktop's** (#2749), in a sentence, where it is
/// another, or could not be read; `None` where it is this desktop's own.
pub fn behind(machine: &Machine, kept: &Kept) -> Option<String> {
    let ours = desktop_version();
    match version_of(machine, kept) {
        Ok(theirs) if theirs == ours => None,
        Ok(theirs) => Some(format!(
            "runner {} runs purlis {theirs}, and this desktop runs {ours}",
            kept.name()
        )),
        Err(why) => Some(format!(
            "runner {}'s version could not be read, so it may not run this desktop's {ours}: \
             {why}",
            kept.name()
        )),
    }
}

/// **The version `kept`'s runner runs**, read once for each container and image and kept for the
/// rest of this run (#2749): a container runs one image by digest, so what it says does not
/// change, and a link need not ask it again. What could not be read is asked again next time.
pub fn version_of(machine: &Machine, kept: &Kept) -> Result<String, String> {
    once_for(
        kept.made.container.as_str(),
        kept.made.runner.image().as_str(),
        || machine.docker.version_of(&kept.made).map_err(|why| why.0),
    )
}

/// What `read` says of the container `container` of `image`, read the first time it answers
/// and kept from then on; one that could not be read is read again next time.
fn once_for(
    container: &str,
    image: &str,
    read: impl FnOnce() -> Result<String, String>,
) -> Result<String, String> {
    use std::collections::HashMap;
    use std::sync::{Mutex, PoisonError};
    static READ: Mutex<Option<HashMap<(String, String), String>>> = Mutex::new(None);
    let key = (container.to_owned(), image.to_owned());
    let kept = READ
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .and_then(|read| read.get(&key).cloned());
    if let Some(kept) = kept {
        return Ok(kept);
    }
    let version = read()?;
    READ.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert_with(HashMap::new)
        .insert(key, version.clone());
    Ok(version)
}

/// **How many chats still run on the runner `to`** (#2749, #2865): what a draining runner waits
/// on before it is removed. One link, made and let go of; a runner that does not answer is not
/// known to be idle.
pub fn chats_on(to: &ToARunner) -> Result<u32, String> {
    count_of(listed(to))
}

/// **How many programs a runner's list holds that have not ended**, or why there is no list, in
/// words shown safely (#2052): what the drain and the removal of a pool say.
pub fn count_of(
    listed: Result<Vec<crate::devices::ListedProgram>, crate::dockerpools::PoolLink>,
) -> Result<u32, String> {
    use crate::dockerpools::PoolLink;
    match listed {
        Ok(listed) => Ok(
            u32::try_from(listed.iter().filter(|program| !program.ended).count())
                .unwrap_or(u32::MAX),
        ),
        Err(PoolLink::Down { why }) => Err(crate::devices::shown_safely(&why)),
        Err(PoolLink::KeyChanged) => {
            Err("the runner answered with another key than the one pinned for it".to_owned())
        }
        Err(PoolLink::OperatorKeyChanged) => Err(
            "the cluster now names another cluster operator key than the one pinned for it"
                .to_owned(),
        ),
        Err(PoolLink::Suspended) => Err("the runner is suspended".to_owned()),
        Err(PoolLink::Linked) => Err("the runner did not list".to_owned()),
    }
}

/// **Runs `then` on the first link to `to`**, on a runtime of its own, waiting at most
/// [`ANSWER_WITHIN`] for the link and for `then`: a link that is up, or why there is none.
fn on_one_link<T>(
    to: &ToARunner,
    then: impl AsyncFnOnce(
        Result<purlis_session_protocol::session::Client, hostlink::Down>,
    ) -> Result<T, String>,
) -> Result<T, String> {
    on_one_link_within(to, ANSWER_WITHIN, then)
}

/// [`on_one_link`], waiting at most `within` for `then`.
fn on_one_link_within<T>(
    to: &ToARunner,
    within: std::time::Duration,
    then: impl AsyncFnOnce(
        Result<purlis_session_protocol::session::Client, hostlink::Down>,
    ) -> Result<T, String>,
) -> Result<T, String> {
    let to = to.clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|why| format!("the link could not be started: {why}"))?;
    runtime.block_on(async move {
        let mut links = to.keep();
        let linked = tokio::time::timeout(ANSWER_WITHIN, links.next())
            .await
            .map_err(|_| "the runner did not answer in time".to_owned())?;
        let client = match linked {
            Some(hostlink::Linked::Up(link)) => {
                Ok(purlis_session_protocol::session::Client::new(*link).0)
            }
            Some(hostlink::Linked::Down(down)) => Err(down),
            None => return Err("the link to the runner ended".to_owned()),
        };
        tokio::time::timeout(within, then(client))
            .await
            .map_err(|_| "the runner did not answer in time".to_owned())?
    })
}

/// **Takes this desktop out of the runner's peers over a link of its own** (`unpair`, 1.25), as
/// the person removes the pool: `Ok` once the runner answered, or why not. A runner that cannot
/// be linked, or whose key changed, is not unpaired; its peers go with its home volume.
pub fn unpair(to: &ToARunner) -> Result<(), String> {
    on_one_link(to, async |linked| match linked {
        Ok(client) => client.unpair().await.map_err(|why| why.to_string()),
        Err(down) => Err(down.why),
    })
    // What the removal says of it: shown safely, as every pool surface shows such words.
    .map_err(|why| crate::devices::shown_safely(&why))
}

/// **A profile a runner kept**: whether it is signed in there, and what of its settings stayed
/// behind, over the limits (#4081).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileKept {
    pub standing: purlis_session_protocol::session::ProfileStanding,
    pub left_behind: Vec<purlis_core::profilestore::onrunner::LeftBehind>,
}

/// What a runner answered each profile it was asked to keep: [`ProfileKept`], or why it was not
/// kept.
pub type ProfilesKept = Vec<Result<ProfileKept, String>>;

/// `why` a runner did not take a call: its own words marked as its and cut short, where it said.
fn refused_by_the_runner(why: &purlis_session_protocol::session::CallError) -> String {
    match why.refusal() {
        Some(refused) => said_by_the_runner(&refused.why),
        None => why.to_string(),
    }
}

/// **`sending`, kept over `client`'s link** (#4081): its batches first, each answered before the
/// next and each given [`ANSWER_WITHIN`], so a large profile is held to a runner that keeps
/// answering, within [`ALL_KEPT_WITHIN`]. The first refused stops them, and no keep is sent: what the link
/// staged is dropped there, so nothing of it is kept.
async fn keep_one(
    client: &purlis_session_protocol::session::Client,
    sending: purlis_core::profilestore::onrunner::ToKeep,
) -> Result<ProfileKept, String> {
    let late = |_| LATE.to_owned();
    for batch in sending.batches {
        tokio::time::timeout(ANSWER_WITHIN, client.stage_profile_settings(batch))
            .await
            .map_err(late)?
            .map_err(|why| refused_by_the_runner(&why))?;
    }
    // Keeping them writes every file, and asks the harness whether it is signed in.
    let within = if sending.profile.settings_staged.is_some() {
        KEPT_WITHIN
    } else {
        ANSWER_WITHIN
    };
    let standing = tokio::time::timeout(within, client.keep_profile(sending.profile))
        .await
        .map_err(late)?
        .map_err(|why| refused_by_the_runner(&why))?;
    Ok(ProfileKept {
        standing,
        left_behind: sending.left_behind,
    })
}

/// **Has the runner keep each of this desktop's `profiles` over one link of its own**
/// (`keep_profile`, 1.43, #2049), in order: whether each is signed in there, or why the runner
/// did not keep it, its own words marked as its and cut short. `None` for a runner of a purlis
/// before 1.43, which is sent nothing and runs chats on profiles of its own. Each call is held to
/// its own time ([`keep_one`]), and all of them together to [`ALL_KEPT_WITHIN`].
pub fn keep_profiles(
    to: &ToARunner,
    profiles: Vec<purlis_core::profilestore::onrunner::Kept>,
) -> Result<Option<ProfilesKept>, String> {
    on_one_link_within(to, ALL_KEPT_WITHIN, async |linked| match linked {
        Ok(client) if !keeps_profiles(client.version()) => Ok(None),
        Ok(client) => {
            let version = client.version();
            let mut said = Vec::with_capacity(profiles.len());
            for kept in &profiles {
                let sending = match for_a_runner_of(version, kept) {
                    Ok(sending) => sending,
                    Err(why) => {
                        said.push(Err(why));
                        continue;
                    }
                };
                let kept = keep_one(&client, sending).await;
                let gone = matches!(&kept, Err(why) if why == LATE);
                said.push(kept);
                // A runner that stopped answering is asked nothing more.
                if gone {
                    break;
                }
            }
            Ok(Some(said))
        }
        Err(down) => Err(down.why),
    })
}

/// **`kept`, as a runner speaking `version` is sent it**: with its settings, read now, only from
/// 1.50 (#3448). A runner before ignores them, so none is read for it, and nothing about them
/// can refuse a chat that starts on it: it keeps the settings it has, as before. A runner of 1.56
/// or later takes them in batches ([`purlis_core::profilestore::onrunner::in_batches`], #4081);
/// one between is sent what one `keep_profile` carries
/// ([`purlis_core::profilestore::onrunner::to_send`]). `Err`, in words, where its settings
/// cannot be sent.
pub fn for_a_runner_of(
    version: purlis_session_protocol::version::Version,
    kept: &purlis_core::profilestore::onrunner::Kept,
) -> Result<purlis_core::profilestore::onrunner::ToKeep, String> {
    use purlis_core::profilestore::onrunner::{ToKeep, in_batches, to_send};
    use purlis_session_protocol::session::{KNOWS_PROFILE_SETTINGS, KNOWS_SETTINGS_IN_BATCHES};
    if version >= KNOWS_SETTINGS_IN_BATCHES {
        return in_batches(kept);
    }
    let profile = if version >= KNOWS_PROFILE_SETTINGS {
        to_send(kept)?
    } else {
        kept.clone().into()
    };
    Ok(ToKeep {
        profile,
        batches: Vec::new(),
        left_behind: Vec::new(),
    })
}

/// Whether a runner speaking `version` keeps a desktop's profiles (`keep_profile`, 1.43).
pub fn keeps_profiles(version: purlis_session_protocol::version::Version) -> bool {
    version >= purlis_session_protocol::session::KNOWS_KEEP_PROFILE
}

/// **Has the runner `to` reaches forget this machine's profile `name`** (`forget_profile`, 1.52,
/// #4047), its folder and sign-in there with it. `None`, with nothing sent, from a runner before
/// 1.52, which keeps it.
pub fn forget_profile(to: &ToARunner, name: &str) -> Result<Option<()>, String> {
    let name = name.to_owned();
    on_one_link(to, async move |linked| match linked {
        Ok(client) if !forgets_profiles(client.version()) => Ok(None),
        Ok(client) => {
            client
                .forget_profile(&name)
                .await
                .map(Some)
                .map_err(|why| match why.refusal() {
                    Some(refused) => said_by_the_runner(&refused.why),
                    None => why.to_string(),
                })
        }
        Err(down) => Err(down.why),
    })
}

/// Whether a runner speaking `version` forgets a desktop's profile (`forget_profile`, 1.52).
pub fn forgets_profiles(version: purlis_session_protocol::version::Version) -> bool {
    version >= purlis_session_protocol::session::KNOWS_FORGET_PROFILE
}

/// A runner's own words refusing a profile, as the person reads them: marked as its, and cut
/// short.
pub fn said_by_the_runner(why: &str) -> String {
    format!("the runner says: {}", purlis_core::shown::short(why))
}

/// **Where the runner stands, as one link to it says** (#2679): linked with the key pinned for
/// it, refused for another key, or down and why.
pub fn link_state(to: &ToARunner) -> Result<crate::dockerpools::PoolLink, String> {
    use crate::dockerpools::PoolLink;
    on_one_link(to, async |linked| {
        Ok(match linked {
            Ok(_) => PoolLink::Linked,
            Err(down) => link_of(down),
        })
    })
    .map_err(|why| crate::devices::shown_safely(&why))
}

/// **Where a runner stands, as a link that is down says** (#2052): its key changed, or down, and
/// why, shown safely ([`crate::devices::shown_safely`]): the reason can carry what the runner's
/// container said.
pub fn link_of(down: hostlink::Down) -> crate::dockerpools::PoolLink {
    use crate::dockerpools::PoolLink;
    if down.key_changed {
        PoolLink::KeyChanged
    } else {
        PoolLink::Down {
            why: crate::devices::shown_safely(&down.why),
        }
    }
}

/// **What the runner `to` holds, as its own list says** (#2052, the Devices view): one link, made
/// and let go of; or where the runner stands where it gave no list, as [`link_state`] says it.
pub fn listed(
    to: &ToARunner,
) -> Result<Vec<crate::devices::ListedProgram>, crate::dockerpools::PoolLink> {
    use crate::dockerpools::PoolLink;
    let listed = on_one_link(to, async |linked| {
        Ok(match linked {
            Ok(client) => match client.list().await {
                Ok(chats) => Ok(chats
                    .into_iter()
                    .map(|chat| crate::devices::ListedProgram {
                        browser: chat.kind
                            == Some(purlis_session_protocol::session::ProgramKind::Browser),
                        ended: chat.state == "ended",
                        id: chat.chat,
                        project: chat.project,
                        number: chat.number,
                    })
                    .collect()),
                // In purlis's words: the runner's own are never shown (#2052).
                Err(why) => Err(PoolLink::Down {
                    why: crate::devices::list_refused(&why),
                }),
            },
            Err(down) => Err(link_of(down)),
        })
    });
    match listed {
        Ok(listed) => listed,
        Err(why) => Err(PoolLink::Down {
            why: crate::devices::shown_safely(&why),
        }),
    }
}

/// How long one link to a runner waits for it to answer.
const ANSWER_WITHIN: std::time::Duration = std::time::Duration::from_secs(20);

/// What a call a runner did not answer in time says.
const LATE: &str = "the runner did not answer in time";

/// How long a runner has to keep a profile's staged settings: it writes each of them.
const KEPT_WITHIN: std::time::Duration = std::time::Duration::from_secs(120);

/// How long one link waits for every profile it keeps, batches and all.
const ALL_KEPT_WITHIN: std::time::Duration = std::time::Duration::from_secs(600);

#[cfg(test)]
mod tests;
