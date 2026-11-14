// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **When a pool's pin no longer holds** (#6270, ADR 0123 §3): the machine shows another host key,
//! its CA file holds another CA, or its engine answers with another id. The pool then starts no
//! runner, opens no new link and sends no vault value, since every run of `docker` for it is
//! refused before it reaches the machine ([`crate::sshroute::pool::docker_of`]); a link already
//! open stays open, held by its runner's own pinned key (ADR 0078).
//!
//! This module names the change ([`of`]) and says it ([`said`]), the same in the window's Notice
//! (cause `pool-pin-changed`) and in `purlis pool doctor`. And it keeps the changes this process
//! found ([`Changes`], [`found`]), so the window shows one without contacting any machine itself:
//! each is found where a run for the pool was refused, and goes once its pin holds again (a run
//! got through, or the person trusted the machine again) or the pool is removed.

use serde::Serialize;

use crate::sshroute::connect::Finding;

/// **What no longer matches the pin.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Changed {
    /// The machine's SSH host key.
    Key,
    /// The CA a machine over Docker's TLS API is verified against.
    Ca,
    /// The engine's id (`docker info`'s `.ID`), or a Podman's socket and store.
    Engine,
}

/// **A pool's pin that no longer holds**, with both fingerprints and the words that say it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PinChange {
    pub pool: String,
    pub host: String,
    pub what: Changed,
    /// What was pinned: a key's or a CA's SHA-256, or the engine's id.
    pub pinned: String,
    /// What the machine shows now, the same way.
    pub now: String,
    /// The Notice's title.
    pub title: String,
    /// The Notice's sentence ([`said`]).
    pub said: String,
}

/// What purlis holds back from a machine whose pin no longer holds, the same for every change.
const HELD_BACK: &str = "purlis starts no runner, opens no new link and sends no vault value there";

/// **The pin change `finding` is, for pool `pool`**: `None` for any finding but a changed key, CA
/// or engine.
#[must_use]
pub fn of(pool: &str, finding: &Finding) -> Option<PinChange> {
    let (host, what, pinned, now, title, said) = match finding {
        Finding::KeyChanged { host, pinned, now } => {
            let (pinned, now) = (pinned.fingerprint(), now.fingerprint());
            (
                host,
                Changed::Key,
                pinned.clone(),
                now.clone(),
                format!("Pool {pool}: {host} shows a different key"),
                format!(
                    "Pool {pool}: {host} shows a different key than the one you trusted ({now} \
                     now, {pinned} trusted). This happens when the machine is reinstalled, and can \
                     mean something else is answering in its place. {HELD_BACK} until you check \
                     its key and trust it again. Links already open stay open."
                ),
            )
        }
        Finding::CaChanged {
            host,
            pinned,
            now,
            file,
        } => (
            host,
            Changed::Ca,
            pinned.clone(),
            now.clone(),
            format!("Pool {pool}: {host}'s CA changed"),
            format!(
                "Pool {pool}: {file} no longer holds the CA you trusted for {host} ({now} now, \
                 {pinned} trusted). This happens when the CA is replaced, and can mean something \
                 else is answering in its place. {HELD_BACK} until you check the CA and trust it \
                 again. Links already open stay open."
            ),
        ),
        Finding::EngineChanged {
            host,
            pinned,
            now,
            podman,
        } => {
            let engine = if *podman { "Podman" } else { "Docker" };
            (
                host,
                Changed::Engine,
                pinned.clone(),
                now.clone(),
                format!("Pool {pool}: {engine} on {host} isn't the one you trusted"),
                format!(
                    "Pool {pool}: {engine} on {host} isn't the one you trusted ({now} now, \
                     {pinned} trusted). This happens when {engine} there is reset or reinstalled. \
                     {HELD_BACK} until you check it and trust it again. Links already open stay \
                     open. The pool's runners are on another engine, the one you trusted. Once \
                     you trust this one, you can make the runner again."
                ),
            )
        }
        _ => return None,
    };
    Some(PinChange {
        pool: pool.to_owned(),
        host: host.clone(),
        what,
        pinned,
        now,
        title,
        said,
    })
}

/// **What a pin change says**: the pool, what changed, both fingerprints, the usual reason, what
/// purlis holds back until the person trusts it again, and, for an engine, where its runners are.
#[must_use]
pub fn said(change: &PinChange) -> String {
    change.said.clone()
}

/// **The pin changes this process found**, one a pool, and who is told each time they change.
#[derive(Default)]
pub struct Changes {
    found: std::sync::Mutex<Vec<PinChange>>,
    tell: Option<Box<dyn Fn() + Send + Sync>>,
}

impl Changes {
    /// These changes, telling `tell` each time they change.
    #[must_use]
    pub fn telling(self, tell: Box<dyn Fn() + Send + Sync>) -> Changes {
        Changes {
            tell: Some(tell),
            ..self
        }
    }

    /// **`change` was found**: it replaces what was found of its pool before. Told only where it
    /// is new.
    pub fn found(&self, change: PinChange) {
        let mut found = self.lock();
        if found.contains(&change) {
            return;
        }
        found.retain(|one| one.pool != change.pool);
        found.push(change);
        drop(found);
        self.told();
    }

    /// **Pool `pool`'s pin holds again**, or the pool is gone: what was found of it goes.
    pub fn holds(&self, pool: &str) {
        let mut found = self.lock();
        let before = found.len();
        found.retain(|one| one.pool != pool);
        let changed = found.len() != before;
        drop(found);
        if changed {
            self.told();
        }
    }

    /// Every change found, in the order they were found.
    #[must_use]
    pub fn all(&self) -> Vec<PinChange> {
        self.lock().clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<PinChange>> {
        self.found
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn told(&self) {
        if let Some(tell) = &self.tell {
            tell();
        }
    }
}

/// This process's changes.
static CHANGES: std::sync::OnceLock<Changes> = std::sync::OnceLock::new();

fn changes() -> &'static Changes {
    CHANGES.get_or_init(Changes::default)
}

/// **Tells the window with `tell` each time a pool's pin is found changed, or holds again.** Set
/// once, before anything is found; a later call changes nothing.
pub fn telling(tell: Box<dyn Fn() + Send + Sync>) {
    let _ = CHANGES.set(Changes::default().telling(tell));
}

/// **What a run for pool `pool` found**: a pin change is kept for the window; any other finding
/// says nothing of the pin, and leaves what was kept.
pub fn found(pool: &str, finding: &Finding) {
    if let Some(change) = of(pool, finding) {
        changes().found(change);
    }
}

/// **Pool `pool`'s pin holds**: a run got through, the person trusted it again, or it is gone.
pub fn holds(pool: &str) {
    changes().holds(pool);
}

/// **Every pool whose pin was found changed in this process**, and still not trusted again.
#[must_use]
pub fn found_now() -> Vec<PinChange> {
    changes().all()
}

/// **What `purlis pool doctor` says of pool `pool`, and its exit**: `reached` is the machine's
/// host where every run got through, else what refused it. A pin change is said in the Notice's
/// own words, with its two ways out; any other finding in Pool setup's. Either exits 1.
#[must_use]
pub fn doctor_said(pool: &str, reached: Result<&str, &Finding>) -> (String, u8) {
    match reached {
        Ok(host) => (
            format!(
                "Pool {pool}: {host} is the machine you trusted, and its engine the one you \
                 trusted.\n"
            ),
            0,
        ),
        Err(finding) => match of(pool, finding) {
            Some(change) => (
                format!(
                    "{}\n{}\nIn the window, open Pool setup on pool {pool} and press Check and \
                     trust again, or remove the pool with Remove pool.\n",
                    change.title,
                    said(&change)
                ),
                1,
            ),
            None => (
                format!("Pool {pool}: {}\n", crate::poolsetup::connect_said(finding)),
                1,
            ),
        },
    }
}

#[cfg(test)]
mod tests;
