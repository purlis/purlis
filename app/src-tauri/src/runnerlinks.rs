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

use std::path::{Path, PathBuf};
use std::sync::Arc;

use purlis_core::dockerpool::Docker;
use purlis_core::runners::Kept;
use purlis_host::CommandConnector;
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

    /// This desktop on a link, its key made on first use.
    pub fn desktop(&self) -> Result<purlis_core::linkkey::Desktop, String> {
        purlis_core::linkkey::desktop(&self.config, &*self.keyring)
    }
}

impl ToARunner {
    /// The runner kept for `pool` on `machine`: only one this machine paired and keeps.
    pub fn of(machine: &Machine, pool: &str) -> Result<ToARunner, String> {
        let kept = purlis_core::runners::runner_for(&machine.config, pool)
            .map_err(|why| why.to_string())?;
        let desktop = machine.desktop()?;
        Ok(ToARunner::kept(
            &kept,
            machine.docker.clone(),
            desktop.into_identity(),
        ))
    }

    fn kept(kept: &Kept, docker: Docker, desktop: Identity) -> ToARunner {
        let made = kept.made.clone();
        let checking = docker.clone();
        let connector = CommandConnector::new(
            docker.0.clone(),
            made.connect_args(),
            // The container is checked as it was made before every connection (ADR 0078 as
            // amended 2026-10-17, item 2): anything else refuses the link and runs nothing.
            Arc::new(move || checking.check(&made).map_err(|why| why.0)),
        );
        ToARunner {
            connector,
            device: desktop.device,
            key: desktop.key,
            pinned: kept.pinned.clone(),
        }
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
    relaying(move || Machine::here(&config_root))
}

/// [`relays_to`], on the machine `machine` answers as each `relay_to` is taken.
pub fn relaying(
    machine: impl Fn() -> Result<Machine, String> + Send + Sync + 'static,
) -> purlis_host::RelaysTo {
    Arc::new(move |pool: &str| {
        let to = ToARunner::of(&machine()?, pool)?;
        Ok(Arc::new(move || to.keep()) as purlis_host::Keep)
    })
}

#[cfg(test)]
mod tests;
