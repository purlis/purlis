// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The `docker` each run for a pool is given** (#6266, ADR 0123 §1 and §8): this machine's own
//! for a local pool, as it always was; for a pool on another machine, the same program pointed at
//! the pool's forward ([`super::forward::hold`]), opened again where it dropped, and only once the
//! engine it reaches answers with the pinned id. The host key is ssh's to refuse as the forward
//! opens; a forward that will not open is looked at again to say why ([`diagnosed`]).

use std::path::{Path, PathBuf};

use super::connect::{self, Finding};
use super::forward::{self, Opening};
use crate::dockerpool::Docker;
use crate::runners::{Pool, Remote};

/// **Where `docker`'s pools keep their forwards' sockets and each run's settings**: beside the
/// settings folder it names (a test's), else `<data>/engine-links` and `<data>/docker-settings`.
pub fn folders_for(docker: &Docker) -> Result<(PathBuf, PathBuf), Finding> {
    match docker.settings_folder() {
        Some(settings_in) => Ok((
            settings_in.parent().unwrap_or(settings_in).join(super::DIR),
            settings_in.to_path_buf(),
        )),
        None => folders(),
    }
}

/// Where this machine keeps the forwards' sockets and each run's settings: `<data>/engine-links`
/// and `<data>/docker-settings`.
pub fn folders() -> Result<(PathBuf, PathBuf), Finding> {
    let data = crate::datahome::root().ok_or_else(|| {
        Finding::Here("purlis has no data folder to keep a pool's SSH forward in".to_owned())
    })?;
    Ok((
        data.join(super::DIR),
        data.join(crate::dockerpool::settings::DIR),
    ))
}

/// **`docker` as pool `pool`'s runs are given it**: itself for a pool on this machine, else
/// pointed at the pool's forward, held open, its engine checked against the pin.
pub fn docker_of(docker: &Docker, pool: &Pool) -> Result<Docker, Finding> {
    let Some(remote) = pool.remote() else {
        return Ok(docker.clone());
    };
    let (engine_links, settings_in) = folders_for(docker)?;
    let ssh = super::ssh_here().map_err(Finding::Here)?;
    docker_through(
        docker,
        &ssh,
        pool.name(),
        remote,
        &engine_links,
        &settings_in,
    )
}

/// [`docker_of`], with its folders and `ssh` named.
pub fn docker_through(
    docker: &Docker,
    ssh: &Path,
    pool: &str,
    remote: &Remote,
    engine_links: &Path,
    settings_in: &Path,
) -> Result<Docker, Finding> {
    let opening = Opening {
        ssh,
        route: &remote.route,
        pool,
        key: &remote.pin.host_key,
        engine_links,
        settings_in,
    };
    let (socket, opened) = forward::hold_noting(&opening)
        .map_err(|said| diagnosed(ssh, remote, pool, settings_in, said))?;
    let through = docker.clone().through(socket);
    if opened || !checked_lately(pool, &remote.pin) {
        if let Err(finding) = connect::engine_holds(&through, &remote.route, &remote.pin) {
            // Nothing more goes over a forward to an engine that isn't the pinned one.
            forget_checked(pool);
            if matches!(finding, Finding::EngineChanged { .. }) {
                forward::let_go(pool);
            }
            return Err(finding);
        }
        note_checked(pool, &remote.pin);
    }
    Ok(through)
}

/// **How long an engine found to be the pinned one is taken as it, over the same forward**: a
/// pool's surfaces ask after its runner often, and each ask would otherwise be a `docker info`
/// over SSH. A forward opened again is checked again at once.
pub const ENGINE_CHECKED_FOR: std::time::Duration = std::time::Duration::from_secs(30);

/// When each pool's engine was last found to be its pin's, in this process.
type Checked = std::collections::BTreeMap<String, (crate::sshroute::Pin, std::time::Instant)>;
static CHECKED: std::sync::Mutex<Checked> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

fn checked_lately(pool: &str, pin: &crate::sshroute::Pin) -> bool {
    CHECKED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(pool)
        .is_some_and(|(checked, at)| checked == pin && at.elapsed() < ENGINE_CHECKED_FOR)
}

fn note_checked(pool: &str, pin: &crate::sshroute::Pin) {
    CHECKED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(pool.to_owned(), (pin.clone(), std::time::Instant::now()));
}

fn forget_checked(pool: &str) {
    CHECKED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(pool);
}

/// **Pool `name`'s `docker` and the socket of the engine it reaches**: for a pool on this
/// machine, `docker` itself and `local`, this machine's engine as it was found; for a pool on
/// another machine, through its forward (held open, engine checked) and the forward's socket.
/// What a sign-in on its runner is bound to (#2049, ADR 0123 §1).
pub fn reached(
    config: &Path,
    docker: &Docker,
    name: &str,
    local: Result<PathBuf, String>,
) -> Result<(Docker, PathBuf), String> {
    let through = docker_of_named(config, docker, name)?;
    match through.forwarded().map(Path::to_path_buf) {
        Some(socket) => Ok((through, socket)),
        None => Ok((through, local?)),
    }
}

/// **Whether the pool named `name` under `config` is on another machine.**
#[must_use]
pub fn is_remote(config: &Path, name: &str) -> bool {
    crate::runners::pools(config)
        .ok()
        .and_then(|pools| pools.into_iter().find(|pool| pool.name() == name))
        .is_some_and(|pool| pool.remote().is_some())
}

/// **Where pool `pool`'s forward has its socket on this machine**, whether or not it is open.
pub fn socket_for(docker: &Docker, pool: &str) -> Result<PathBuf, Finding> {
    let (engine_links, _) = folders_for(docker)?;
    super::socket_of(&engine_links, pool).map_err(Finding::Here)
}

/// **[`docker_of`] for the pool named `name`** under `config`, in words: what a link, a check or
/// a make reads before it runs anything there.
pub fn docker_of_named(config: &Path, docker: &Docker, name: &str) -> Result<Docker, String> {
    let pools = crate::runners::pools(config).map_err(|why| why.to_string())?;
    match pools.iter().find(|pool| pool.name() == name) {
        Some(pool) => {
            docker_of(docker, pool).map_err(|finding| crate::poolsetup::connect_said(&finding))
        }
        // A runner whose pool is gone is reached as it always was.
        None => Ok(docker.clone()),
    }
}

/// **Why a forward did not open**, looked at again from the start (ADR 0123 §7): the name, the
/// machine answering, the key it presents against the pin, the sign-in. Where all of those hold,
/// what ssh said.
fn diagnosed(ssh: &Path, remote: &Remote, pool: &str, settings_in: &Path, said: String) -> Finding {
    let route = &remote.route;
    let resolved = match connect::resolve(ssh, route) {
        Ok(resolved) => resolved,
        Err(finding) => return finding,
    };
    if let Err(finding) = connect::reachable(route, &resolved) {
        return finding;
    }
    match connect::meet(ssh, route, settings_in) {
        Ok(now) if now != remote.pin.host_key => {
            return Finding::KeyChanged {
                host: route.host().to_owned(),
                pinned: remote.pin.host_key.clone(),
                now,
            };
        }
        Ok(_) => {}
        Err(finding) => return finding,
    }
    match connect::probe(
        ssh,
        route,
        pool,
        &remote.pin.host_key,
        settings_in,
        &resolved.user,
    ) {
        Ok(_) => Finding::EngineSilent {
            host: route.host().to_owned(),
            said,
        },
        Err(finding) => finding,
    }
}
