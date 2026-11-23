// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The pools' pairing, read by the route** (one login, slice J; ADR 0128 D9): which runners
//! this desktop paired, and which project a paired runner's chat belongs to. The desktop side of
//! [`RunnerLink`]; the forwarding's channel is ADR 0078's and is not here.
//!
//! - **Paired** is the device id and key this desktop pinned at the runner's pairing, in the
//!   machine store ([`purlis_core::runners`]): a Docker pool's runner, a cluster pool's, and a
//!   chat's fresh runner. A runner that answered that this desktop is out of its peers is not.
//! - **The project is the pairing's, never the caller's.** Only a chat's fresh runner records one
//!   (the checkout its chat started from, [`purlis_core::runners::FreshOf::root`]). A pool's own
//!   runner serves any project's chats, so no pairing names one: it is refused, and its chats
//!   keep their token connections. A fresh runner's row with no checkout (written before it) is
//!   refused the same way.
//!
//! **Tier** (ADR 0069): nothing is stored here; the pairings are read from the machine store on
//! each ask, so a pairing made or removed while the app runs is in force at once.

use std::path::{Path, PathBuf};

use super::{RunnerLink, RunnerPeer};

/// One pinned runner: who it is on a link, and the project its pairing names, if it names one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pairing {
    pub device: String,
    pub key: [u8; 32],
    pub project: Option<PathBuf>,
}

/// The desktop's [`RunnerLink`]: the pairings it reads, whenever it is asked.
pub struct DesktopPairings {
    read: Box<dyn Fn() -> Vec<Pairing> + Send + Sync>,
}

impl DesktopPairings {
    /// A link reading `read` on each ask.
    pub fn new(read: impl Fn() -> Vec<Pairing> + Send + Sync + 'static) -> Self {
        Self {
            read: Box::new(read),
        }
    }

    /// The pairings in the machine store under `config`.
    pub fn from_store(config: &Path) -> Self {
        let config = config.to_path_buf();
        Self::new(move || read_store(&config))
    }

    fn of(&self, peer: &RunnerPeer) -> Option<Pairing> {
        (self.read)()
            .into_iter()
            .find(|pairing| pairing.device == peer.device && pairing.key == peer.key)
    }
}

/// Every runner the store pins. A store that cannot be read pairs none: nothing is forwarded.
fn read_store(config: &Path) -> Vec<Pairing> {
    use purlis_core::runners;
    let pinned =
        |device: &str, key: &purlis_session_protocol::hostlink::PublicKey, project| Pairing {
            device: device.to_owned(),
            key: *key.as_bytes(),
            project,
        };
    let mut all = Vec::new();
    for kept in runners::read(config).unwrap_or_default() {
        if !kept.unpaired {
            all.push(pinned(&kept.pinned.device, &kept.pinned.key, None));
        }
    }
    for kept in runners::cluster_runners(config).unwrap_or_default() {
        all.push(pinned(&kept.pinned.device, &kept.pinned.key, None));
    }
    for kept in runners::fresh_runners(config).unwrap_or_default() {
        let project = kept
            .of
            .as_ref()
            .and_then(|of| of.root.as_deref())
            .map(PathBuf::from);
        all.push(pinned(&kept.pinned.device, &kept.pinned.key, project));
    }
    all
}

impl RunnerLink for DesktopPairings {
    fn paired(&self, peer: &RunnerPeer) -> bool {
        self.of(peer).is_some()
    }

    fn project_of(&self, peer: &RunnerPeer, _run: &str) -> Option<PathBuf> {
        self.of(peer)?.project
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(device: &str, key: u8) -> RunnerPeer {
        RunnerPeer {
            device: device.to_owned(),
            key: [key; 32],
        }
    }

    fn pairing(device: &str, key: u8, project: Option<&str>) -> Pairing {
        Pairing {
            device: device.to_owned(),
            key: [key; 32],
            project: project.map(PathBuf::from),
        }
    }

    fn link(pairings: Vec<Pairing>) -> DesktopPairings {
        DesktopPairings::new(move || pairings.clone())
    }

    #[test]
    fn a_runner_this_desktop_pinned_is_paired() {
        let link = link(vec![pairing("r1", 1, None)]);
        assert!(link.paired(&peer("r1", 1)));
    }

    #[test]
    fn a_runner_nobody_pinned_is_not_paired() {
        assert!(!link(vec![]).paired(&peer("r1", 1)));
        assert!(!link(vec![pairing("r2", 1, None)]).paired(&peer("r1", 1)));
    }

    #[test]
    fn a_known_device_with_another_key_is_not_paired() {
        let link = link(vec![pairing("r1", 1, Some("/p"))]);
        assert!(!link.paired(&peer("r1", 2)));
        assert_eq!(link.project_of(&peer("r1", 2), "run"), None);
    }

    #[test]
    fn a_fresh_runners_project_is_the_checkout_its_pairing_names() {
        let link = link(vec![pairing("r1", 1, Some("/work/app"))]);
        assert_eq!(
            link.project_of(&peer("r1", 1), "any-run"),
            Some(PathBuf::from("/work/app"))
        );
    }

    #[test]
    fn a_pools_own_runner_names_no_project_so_none_is_served() {
        let link = link(vec![pairing("r1", 1, None)]);
        assert!(link.paired(&peer("r1", 1)));
        assert_eq!(link.project_of(&peer("r1", 1), "run"), None);
    }

    #[test]
    fn a_pairing_removed_while_the_app_runs_is_gone_at_the_next_ask() {
        use std::sync::{Arc, Mutex};
        let held = Arc::new(Mutex::new(vec![pairing("r1", 1, None)]));
        let seen = Arc::clone(&held);
        let link = DesktopPairings::new(move || seen.lock().unwrap().clone());
        assert!(link.paired(&peer("r1", 1)));
        held.lock().unwrap().clear();
        assert!(!link.paired(&peer("r1", 1)));
    }
}
