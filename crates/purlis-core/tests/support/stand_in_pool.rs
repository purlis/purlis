// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A stand-in pool with one switch per ownership outcome** (R-702, T1a-50), for the runner
//! gate rows of set (iii) (T1a-53, T1a-54, T1a-56).
//!
//! The outcomes are the ones the built ownership check gives (ADR 0078, amended 2026-10-26,
//! item 2; `purlis_core::poollogins::Bound`):
//!
//! - [`Switch::OwnedDocker`]: a Docker pool whose engine is a local socket on this machine;
//! - [`Switch::MarkedCluster`]: a cluster pool the person marked as their own, whose claim the
//!   operator vouches for now;
//! - [`Switch::AnyOther`]: a Docker pool on an engine elsewhere, which nothing shows is theirs.
//!
//! Every field is public and set on its own, so a test changes one at a time: the identity (pool
//! name, context, cluster uid and operator key; or socket and pool name), the per-run claim
//! (claim uid, pod, device and link key), the protocol version and the team-image flag. Each
//! [`StandInPool::judged`] writes the pool and its runner as this machine keeps them, from the
//! fields as they are then, into a folder of its own, and asks the built check. The operator's
//! vouch is the stand-in's: it answers for the claim's pod, with no cluster asked. No Docker
//! engine or cluster is needed.
//!
//! The protocol version and the team-image flag are carried, not judged: no check reads them
//! yet, and the tickets that add one (T1a-54, T1a-53) read them from here.

use std::path::{Path, PathBuf};

use purlis_core::clusterpool::{Claimed, OperatorKey, PoolPin};
use purlis_core::dockerpool::{ContainerId, Image, Made, Runner, User};
use purlis_core::poollogins::{Bound, NotSent};
use purlis_core::runners::{self, ClusterKept, ClusterPool, Pool, Set};
use purlis_session_protocol::hostlink::{Pinned, PublicKey};
use purlis_session_protocol::version::Version;

/// Which ownership outcome the pool is made for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Switch {
    OwnedDocker,
    MarkedCluster,
    AnyOther,
}

/// What the built check judged the pool's runner to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The person's own Docker pool, on this machine's engine.
    Owned,
    /// A cluster pool the person marked as their own, vouched for now.
    Marked,
    /// Nothing shows it is theirs.
    NotOwned,
}

/// Who the pool is: its name, and where it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub pool: String,
    /// A cluster pool's kubeconfig context.
    pub context: String,
    /// A cluster pool's cluster: the uid of its `kube-system` namespace.
    pub cluster_uid: String,
    /// A cluster pool's operator key, as its public key's bytes.
    pub operator_key: [u8; 32],
    /// A Docker pool's engine socket on this machine.
    pub socket: PathBuf,
}

/// What one run's runner is claimed and linked as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub claim_uid: String,
    pub pod: String,
    /// The runner's device id.
    pub device: String,
    /// The runner's link key, as its public key's bytes.
    pub link_key: [u8; 32],
}

/// **A pool for the runner tests**, made for one ownership outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandInPool {
    pub switch: Switch,
    pub identity: Identity,
    pub claim: Claim,
    /// The session protocol version its runner speaks.
    pub protocol: Version,
    /// Whether its runner runs the team's image rather than purlis's own.
    pub team_image: bool,
}

/// The desktop the runners are paired with.
const DESKTOP: &str = "01JA0DESKT0P00000000000000";

impl StandInPool {
    /// A pool made for `switch`, every other field at its default.
    pub fn new(switch: Switch) -> Self {
        StandInPool {
            switch,
            identity: Identity {
                pool: "stand-in".into(),
                context: "stand-in".into(),
                cluster_uid: "7c1e2f4a-3b5d-4e6f-8a9b-0c1d2e3f4a5b".into(),
                operator_key: [3; 32],
                socket: "/var/run/docker.sock".into(),
            },
            claim: Claim {
                claim_uid: "5f0e1d2c-3b4a-4958-8776-655443322110".into(),
                pod: "stand-in-runner-0".into(),
                device: "01JB0RUNNER000000000000000".into(),
                link_key: [1; 32],
            },
            protocol: purlis_session_protocol::session::VERSION,
            team_image: false,
        }
    }

    /// The names of the fields set otherwise than in `other`, as a test names them.
    pub fn fields_differing_from(&self, other: &StandInPool) -> Vec<&'static str> {
        let (a, b) = (&self.identity, &other.identity);
        let (c, d) = (&self.claim, &other.claim);
        [
            ("switch", self.switch != other.switch),
            ("pool name", a.pool != b.pool),
            ("context", a.context != b.context),
            ("cluster uid", a.cluster_uid != b.cluster_uid),
            ("operator key", a.operator_key != b.operator_key),
            ("socket", a.socket != b.socket),
            ("claim uid", c.claim_uid != d.claim_uid),
            ("pod", c.pod != d.pod),
            ("device", c.device != d.device),
            ("link key", c.link_key != d.link_key),
            ("protocol", self.protocol != other.protocol),
            ("team image", self.team_image != other.team_image),
        ]
        .into_iter()
        .filter_map(|(name, differs)| differs.then_some(name))
        .collect()
    }

    /// What the built ownership check judges the pool's runner to be, as its fields are now.
    pub fn judged(&self) -> Outcome {
        let config = tempfile::tempdir().unwrap();
        match self.bound(config.path()) {
            Ok(bound) if bound.on_a_cluster() => Outcome::Marked,
            Ok(_) => Outcome::Owned,
            Err(NotSent::NotYours(_)) => Outcome::NotOwned,
            Err(NotSent::Unconfirmed(why)) => {
                panic!(
                    "the stand-in pool ({:?}) was not judged: {why}",
                    self.switch
                )
            }
        }
    }

    /// The check's answer for the pool, kept under `config` as its fields are now.
    pub fn bound(&self, config: &Path) -> Result<Bound, NotSent> {
        let pool = self.identity.pool.as_str();
        match self.switch {
            Switch::OwnedDocker => Bound::over_the_link_with_kinded(
                Ok(self.keep_docker(config)),
                config,
                pool,
                Some(&format!("{pool}/{pool}")),
            ),
            Switch::AnyOther => {
                self.keep_docker(config);
                Bound::over_the_link_with_kinded(
                    Err("Docker's engine is tcp://build-box.example:2376".to_owned()),
                    config,
                    pool,
                    Some(&format!("{pool}/{pool}")),
                )
            }
            Switch::MarkedCluster => {
                let reached = self.keep_cluster(config).reached();
                let pod_uid = format!("uid-of-{}", self.claim.pod);
                Bound::of_a_cluster_pool_kinded(config, pool, Some(&reached), &|_, _| {
                    Ok(pod_uid.clone())
                })
                .map(|bound| bound.expect("the stand-in's cluster pool is kept"))
            }
        }
    }

    /// The runner as its pairing answered it.
    fn pinned(&self) -> Pinned {
        Pinned {
            device: self.claim.device.clone(),
            key: PublicKey::from_bytes(self.claim.link_key),
        }
    }

    /// Keeps the pool as a Docker pool with its runner under `config`; its engine's socket.
    fn keep_docker(&self, config: &Path) -> PathBuf {
        let name = self.identity.pool.as_str();
        let image = Image::pinned(&format!(
            "ghcr.io/purlis/purlis-runner@sha256:{}",
            "a".repeat(64)
        ))
        .unwrap();
        runners::record_pool(
            config,
            &Pool::new(name, image.clone(), Set::default()).unwrap(),
        )
        .unwrap();
        let made = Made {
            container: ContainerId::new(&"e".repeat(64)).unwrap(),
            runner: Runner::new(
                name,
                image,
                User::new(10001, 10001).unwrap(),
                "/home/purlis",
            )
            .unwrap(),
        };
        runners::keep_for_its_pool(
            config,
            &made,
            &self.pinned(),
            &desktop(),
            &runners::lock(config),
        )
        .unwrap();
        self.identity.socket.clone()
    }

    /// Keeps the pool as a cluster pool the person marked, with its runner, under `config`.
    fn keep_cluster(&self, config: &Path) -> ClusterKept {
        let name = self.identity.pool.as_str();
        let pin = PoolPin {
            key: OperatorKey::of_public(self.identity.operator_key),
            cluster: self.identity.cluster_uid.clone(),
        };
        let pool = ClusterPool::new(
            name,
            Path::new("/home/stand-in/.kube/config"),
            &self.identity.context,
            "stand-in",
            "default",
            pin,
        )
        .unwrap();
        runners::record_cluster_pool_locked(config, &pool, &runners::lock(config)).unwrap();
        runners::set_cluster_owned(config, name, true).unwrap();
        let kept = ClusterKept {
            name: name.to_owned(),
            claimed: Claimed {
                namespace: "default".into(),
                name: name.to_owned(),
                uid: self.claim.claim_uid.clone(),
                requester: "stand-in".into(),
            },
            pod: self.claim.pod.clone(),
            pinned: self.pinned(),
            paired: 0,
            chat: None,
            of: None,
        };
        runners::keep_cluster_runner_locked(config, &kept, &desktop(), &runners::lock(config))
            .unwrap();
        kept
    }
}

/// The desktop's own device id and link key, which no runner may hold.
fn desktop() -> Pinned {
    Pinned {
        device: DESKTOP.into(),
        key: PublicKey::from_bytes([0xd5; 32]),
    }
}
