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
    let (engine_links, settings_in) = folders()?;
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
    let socket =
        forward::hold(&opening).map_err(|said| diagnosed(ssh, remote, pool, settings_in, said))?;
    let through = docker.clone().through(socket);
    if let Err(finding) = connect::engine_holds(&through, &remote.route, &remote.pin) {
        // Nothing more goes over a forward to an engine that isn't the pinned one.
        if matches!(finding, Finding::EngineChanged { .. }) {
            forward::let_go(pool);
        }
        return Err(finding);
    }
    Ok(through)
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
