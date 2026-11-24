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
//!   (the checkout its chat started from, [`purlis_core::runners::FreshOf::root`]), and only
//!   while that checkout still holds the project the row names, by id (as `FreshOf::is_of` holds
//!   it): a project moved away names none, and one replaced at the same path names none **only
//!   where it holds a stored `[project] id`**; with none stored, `id_to_hold_by` is a hash of the
//!   path, which a replacement at that path shares. A known limit, the same as
//!   `FreshOf::is_of`'s. A pool's own runner serves any project's chats, so no pairing names one: it is refused, and its chats keep
//!   their token connections. A fresh runner's row with no checkout (written before it) is
//!   refused the same way.
//! - **The chat is the pairing's too.** A fresh runner is its one chat's, so it is served for
//!   that chat alone; where several rows match a device and key and disagree on project or chat,
//!   none is served.
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
    /// The chat a fresh runner was made for; none on a pool's own runner.
    pub chat: Option<String>,
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

    /// The pairing of `peer`: none where it has none, or where its rows disagree on project or
    /// chat, since nothing says which is right.
    fn of(&self, peer: &RunnerPeer) -> Option<Pairing> {
        let mut matching = (self.read)()
            .into_iter()
            .filter(|pairing| pairing.device == peer.device && pairing.key == peer.key);
        let first = matching.next()?;
        matching
            .all(|other| other.project == first.project && other.chat == first.chat)
            .then_some(first)
    }
}

/// Every runner the store pins. A store that cannot be read pairs none: nothing is forwarded.
fn read_store(config: &Path) -> Vec<Pairing> {
    use purlis_core::runners;
    let pinned = |pinned: &purlis_session_protocol::hostlink::Pinned, project, chat| Pairing {
        device: pinned.device.clone(),
        key: *pinned.key.as_bytes(),
        project,
        chat,
    };
    let mut all = Vec::new();
    for kept in runners::read(config).unwrap_or_default() {
        if !kept.unpaired {
            all.push(pinned(&kept.pinned, None, None));
        }
    }
    for kept in runners::cluster_runners(config).unwrap_or_default() {
        all.push(pinned(&kept.pinned, None, None));
    }
    for kept in runners::fresh_runners(config).unwrap_or_default() {
        // The checkout counts only while it still holds the project the row names, by id: a
        // row outlives a project moved away, or (with a stored `[project] id`) one replaced at
        // the same path. With none stored the id is a path hash: a known limit, as in `is_of`.
        let project = kept.of.as_ref().and_then(|of| {
            let root = Path::new(of.root.as_deref()?);
            (purlis_core::scaffold::planefile::id_to_hold_by(root).ok()? == of.plane)
                .then(|| root.to_path_buf())
        });
        all.push(pinned(&kept.pinned, project, kept.chat));
    }
    all
}

impl RunnerLink for DesktopPairings {
    fn paired(&self, peer: &RunnerPeer) -> bool {
        self.of(peer).is_some()
    }

    fn project_of(&self, peer: &RunnerPeer, chat: &str, _run: &str) -> Option<PathBuf> {
        let pairing = self.of(peer)?;
        // The runner is its chat's alone: another chat's name is not its own.
        (pairing.chat.as_deref() == Some(chat))
            .then_some(pairing.project)
            .flatten()
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

    fn pairing(device: &str, key: u8, project: Option<&str>, chat: Option<&str>) -> Pairing {
        Pairing {
            device: device.to_owned(),
            key: [key; 32],
            project: project.map(PathBuf::from),
            chat: chat.map(str::to_owned),
        }
    }

    fn link(pairings: Vec<Pairing>) -> DesktopPairings {
        DesktopPairings::new(move || pairings.clone())
    }

    #[test]
    fn a_runner_this_desktop_pinned_is_paired() {
        let link = link(vec![pairing("r1", 1, None, None)]);
        assert!(link.paired(&peer("r1", 1)));
    }

    #[test]
    fn a_runner_nobody_pinned_is_not_paired() {
        assert!(!link(vec![]).paired(&peer("r1", 1)));
        assert!(!link(vec![pairing("r2", 1, None, None)]).paired(&peer("r1", 1)));
    }

    #[test]
    fn a_known_device_with_another_key_is_not_paired() {
        let link = link(vec![pairing("r1", 1, Some("/p"), Some("c"))]);
        assert!(!link.paired(&peer("r1", 2)));
        assert_eq!(link.project_of(&peer("r1", 2), "c", "run"), None);
    }

    #[test]
    fn a_fresh_runners_project_is_the_checkout_its_pairing_names() {
        let link = link(vec![pairing("r1", 1, Some("/work/app"), Some("c"))]);
        assert_eq!(
            link.project_of(&peer("r1", 1), "c", "any-run"),
            Some(PathBuf::from("/work/app"))
        );
    }

    #[test]
    fn a_fresh_runner_serves_its_own_chat_and_no_other() {
        let link = link(vec![pairing("r1", 1, Some("/work/app"), Some("x"))]);
        assert_eq!(link.project_of(&peer("r1", 1), "y", "run"), None);
        assert!(link.project_of(&peer("r1", 1), "x", "run").is_some());
    }

    #[test]
    fn a_pools_own_runner_names_no_project_so_none_is_served() {
        let link = link(vec![pairing("r1", 1, None, None)]);
        assert!(link.paired(&peer("r1", 1)));
        assert_eq!(link.project_of(&peer("r1", 1), "c", "run"), None);
    }

    #[test]
    fn rows_of_one_runner_that_disagree_on_project_or_chat_pair_none() {
        for rows in [
            vec![
                pairing("r1", 1, Some("/a"), Some("c")),
                pairing("r1", 1, Some("/b"), Some("c")),
            ],
            vec![
                pairing("r1", 1, Some("/a"), Some("c")),
                pairing("r1", 1, Some("/a"), Some("d")),
            ],
            vec![
                pairing("r1", 1, None, None),
                pairing("r1", 1, Some("/a"), Some("c")),
            ],
        ] {
            let link = link(rows);
            assert!(!link.paired(&peer("r1", 1)));
            assert_eq!(link.project_of(&peer("r1", 1), "c", "run"), None);
        }
        // Rows that agree are one pairing.
        let same = link(vec![
            pairing("r1", 1, Some("/a"), Some("c")),
            pairing("r1", 1, Some("/a"), Some("c")),
        ]);
        assert_eq!(
            same.project_of(&peer("r1", 1), "c", "run"),
            Some(PathBuf::from("/a"))
        );
    }

    #[test]
    fn a_pairing_removed_while_the_app_runs_is_gone_at_the_next_ask() {
        use std::sync::{Arc, Mutex};
        let held = Arc::new(Mutex::new(vec![pairing("r1", 1, None, None)]));
        let seen = Arc::clone(&held);
        let link = DesktopPairings::new(move || seen.lock().unwrap().clone());
        assert!(link.paired(&peer("r1", 1)));
        held.lock().unwrap().clear();
        assert!(!link.paired(&peer("r1", 1)));
    }

    // ---- read_store, over a temporary config root ----

    const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const DOCKER: &str = "01JB0RUNNER0000000000000001";
    const DOCKER_GONE: &str = "01JB0RUNNER0000000000000002";
    const POOL: &str = "01JB0RUNNER0000000000000003";
    const FRESH: &str = "01JB0RUNNER0000000000000004";

    fn docker_row(name: &str, device: &str, key: u8, unpaired: bool) -> serde_json::Value {
        let mut row = serde_json::json!({
            "name": name, "pool": "docker", "container": "a".repeat(64), "image": IMAGE,
            "user": "10001:10001", "home": "/home/purlis", "device": device,
            "key": format!("{key:02x}").repeat(32), "paired": 1,
        });
        if unpaired {
            row["unpaired"] = true.into();
        }
        row
    }

    fn cluster_row(device: &str, key: u8) -> serde_json::Value {
        serde_json::json!({
            "name": "team", "pool": "cluster", "namespace": "ada", "claim": "team-1",
            "claimUid": "6f1c2b8e-0f43-4c2e-9d7a-1b2c3d4e5f60", "requester": "ada@example.com",
            "pod": "team-1", "device": device, "key": format!("{key:02x}").repeat(32),
            "paired": 1_800_000_000_u64,
        })
    }

    fn fresh_row(
        device: &str,
        key: u8,
        chat: &str,
        plane: &str,
        root: Option<&Path>,
    ) -> serde_json::Value {
        let mut row = cluster_row(device, key);
        row["chat"] = chat.into();
        row["plane"] = plane.into();
        row["workspace"] = "main".into();
        if let Some(root) = root {
            row["root"] = root.to_str().unwrap().into();
        }
        row
    }

    fn store(config: &Path, runners: Vec<serde_json::Value>) {
        let folder =
            purlis_same_user::PrivateDirectory::open(&purlis_core::machine::dir(config)).unwrap();
        let file = serde_json::json!({
            "version": 1,
            "pools": [
                { "name": "first", "kind": "docker", "image": IMAGE },
                {
                    "name": "team", "kind": "cluster", "kubeconfig": "/home/ada/.kube/config",
                    "context": "team", "pool": "default", "namespace": "ada",
                    "operatorKey": format!("ed25519:{}", "ab".repeat(32)),
                    "cluster": "3e9a4b2c-7d10-4f5e-8a6b-9c0d1e2f3a4b",
                    "holds": ["/opt/homebrew/bin/kubectl"],
                },
            ],
            "runners": runners,
        });
        folder
            .replace_file(
                purlis_core::machine::RUNNERS_FILE,
                file.to_string().as_bytes(),
            )
            .unwrap();
    }

    fn found(all: &[Pairing], device: &str) -> Pairing {
        all.iter()
            .find(|pairing| pairing.device == device)
            .unwrap_or_else(|| panic!("{device} in {all:?}"))
            .clone()
    }

    #[test]
    fn the_store_pairs_a_docker_runner_unless_it_answered_that_this_desktop_is_out() {
        let config = tempfile::tempdir().unwrap();
        store(
            config.path(),
            vec![
                docker_row("first", DOCKER, 1, false),
                docker_row("first-01234567", DOCKER_GONE, 2, true),
            ],
        );
        let all = read_store(config.path());
        assert_eq!(all.len(), 1, "{all:?}");
        assert_eq!(found(&all, DOCKER).project, None);
    }

    #[test]
    fn the_stores_pool_and_cluster_runners_name_no_project_or_chat() {
        let config = tempfile::tempdir().unwrap();
        store(
            config.path(),
            vec![docker_row("first", DOCKER, 1, false), cluster_row(POOL, 3)],
        );
        let all = read_store(config.path());
        assert_eq!(all.len(), 2, "{all:?}");
        for pairing in &all {
            assert_eq!((&pairing.project, &pairing.chat), (&None, &None));
        }
    }

    #[test]
    fn a_fresh_runner_names_its_chat_and_the_checkout_that_still_holds_its_project() {
        let config = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let plane = purlis_core::scaffold::planefile::id_to_hold_by(root.path()).unwrap();
        store(
            config.path(),
            vec![fresh_row(FRESH, 4, "chat-x", &plane, Some(root.path()))],
        );
        let pairing = found(&read_store(config.path()), FRESH);
        assert_eq!(pairing.project.as_deref(), Some(root.path()));
        assert_eq!(pairing.chat.as_deref(), Some("chat-x"));
    }

    #[test]
    fn a_fresh_runner_whose_checkout_moved_away_or_holds_another_project_names_none() {
        let config = tempfile::tempdir().unwrap();
        let there = tempfile::tempdir().unwrap();
        let plane = purlis_core::scaffold::planefile::id_to_hold_by(there.path()).unwrap();
        let replaced = tempfile::tempdir().unwrap();
        let gone = there.path().join("moved-away");
        store(
            config.path(),
            vec![
                // The row names a project that another now holds at that path.
                fresh_row(FRESH, 4, "chat-x", &plane, Some(replaced.path())),
                // The row's checkout no longer exists.
                fresh_row(POOL, 5, "chat-y", &plane, Some(&gone)),
            ],
        );
        // Two fresh rows on one pool are one pool's two chats: both are kept.
        let all = read_store(config.path());
        for device in [FRESH, POOL] {
            assert_eq!(found(&all, device).project, None, "{device}");
        }
    }

    #[test]
    fn a_fresh_runner_with_no_checkout_named_has_no_project() {
        let config = tempfile::tempdir().unwrap();
        store(
            config.path(),
            vec![fresh_row(
                FRESH,
                4,
                "chat-x",
                "01JB0PLANE00000000000000AA",
                None,
            )],
        );
        let pairing = found(&read_store(config.path()), FRESH);
        assert_eq!(pairing.project, None);
        assert_eq!(pairing.chat.as_deref(), Some("chat-x"));
    }

    #[test]
    fn a_store_that_is_missing_or_not_as_purlis_writes_it_pairs_none() {
        let config = tempfile::tempdir().unwrap();
        assert!(read_store(config.path()).is_empty(), "no file");

        let folder =
            purlis_same_user::PrivateDirectory::open(&purlis_core::machine::dir(config.path()))
                .unwrap();
        for text in ["not json", r#"{"version": 2, "runners": []}"#] {
            folder
                .replace_file(purlis_core::machine::RUNNERS_FILE, text.as_bytes())
                .unwrap();
            assert!(read_store(config.path()).is_empty(), "{text}");
        }
    }
}
