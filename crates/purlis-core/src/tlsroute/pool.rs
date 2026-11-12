// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The `docker` each run for a pool reached over Docker's TLS API is given** (#6267, ADR 0123
//! §1, §3 and §8): this machine's own program, pointed at the pool's route, with its files checked
//! against the pinned CA, and only once the engine answers with the pinned id. Where it doesn't
//! answer, purlis's own handshake says which TLS check failed ([`super::handshake`]).

use std::time::{Duration, Instant};

use super::{Pin, Remote, Through};
use crate::dockerpool::Docker;
use crate::sshroute::connect::Finding;

/// How long an engine found to be the pinned one is taken as it: as for SSH
/// ([`crate::sshroute::pool::ENGINE_CHECKED_FOR`]).
pub const ENGINE_CHECKED_FOR: Duration = crate::sshroute::pool::ENGINE_CHECKED_FOR;

type Checked = std::collections::BTreeMap<String, (Pin, Instant)>;
static CHECKED: std::sync::Mutex<Checked> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// **`docker` as pool `pool`'s runs are given it**, reaching `remote`'s engine over TLS: the
/// files checked against the pin first (a CA changed refuses at once), and the engine's id
/// against the pin at most every [`ENGINE_CHECKED_FOR`].
pub fn docker_over(docker: &Docker, pool: &str, remote: &Remote) -> Result<Docker, Finding> {
    let through = Through::of(remote);
    let material = through.material()?;
    let docker = docker.clone().over_tls(through);
    if checked_lately(pool, &remote.pin) {
        return Ok(docker);
    }
    match super::engine_holds(&docker, &remote.route, &remote.pin) {
        Ok(()) => {
            note_checked(pool, &remote.pin);
            Ok(docker)
        }
        Err(Finding::EngineSilent { host, said }) => {
            forget_checked(pool);
            // Why it didn't answer, by a handshake of purlis's own: which TLS check failed.
            Err(
                match super::handshake(&remote.route, &remote.files, &material) {
                    Ok(()) => Finding::EngineSilent { host, said },
                    Err(finding) => finding,
                },
            )
        }
        Err(finding) => {
            forget_checked(pool);
            Err(finding)
        }
    }
}

fn checked_lately(pool: &str, pin: &Pin) -> bool {
    CHECKED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(pool)
        .is_some_and(|(checked, at)| checked == pin && at.elapsed() < ENGINE_CHECKED_FOR)
}

fn note_checked(pool: &str, pin: &Pin) {
    CHECKED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(pool.to_owned(), (pin.clone(), Instant::now()));
}

fn forget_checked(pool: &str) {
    CHECKED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(pool);
}

/// **Every engine is checked again at its next ask**, and every vault key read again: the person
/// pressed Check again in Pool setup.
pub fn look_again() {
    CHECKED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
    super::forget_vault_keys();
}
