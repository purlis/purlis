// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The `docker` each run for a pool is given** (#6266, ADR 0123 §1 and §8): this machine's own
//! for a local pool, as it always was; for a pool on another machine, the same program pointed at
//! the pool's forward ([`super::forward::hold`]), opened again where it dropped, and only once the
//! engine it reaches answers with the pinned id. The host key is ssh's to refuse as the forward
//! opens; a forward that will not open is looked at again to say why ([`diagnosed`]).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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
    // A forward that keeps failing is tried at most once a wait, and looked at again at most
    // once in [`FINDING_KEPT_FOR`]: each look meets the machine several times.
    let kept = failing(pool, &remote.pin);
    if let Some(Ask::Answer(finding)) = kept.as_ref().map(|kept| kept.ask(Instant::now())) {
        return Err(finding);
    }
    let (socket, opened) = match forward::hold_noting(&opening) {
        Ok(held) => {
            if kept.is_some() {
                forget_failing(pool);
            }
            held
        }
        Err(said) => {
            let now = Instant::now();
            let failed = match kept {
                Some(kept) if !kept.looks_again(now) => kept.failed_again(now, None),
                Some(kept) => {
                    kept.failed_again(now, Some(diagnosed(ssh, remote, pool, settings_in, said)))
                }
                None => Failing::first(diagnosed(ssh, remote, pool, settings_in, said), now),
            };
            let finding = failed.finding.clone();
            note_failing(pool, &remote.pin, failed);
            return Err(finding);
        }
    };
    let through = pointed(docker, ssh, pool, remote, socket, settings_in)?;
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

/// **`docker`, as pool `pool` on another machine runs it, pointed at `socket`** (#6269): Docker's
/// client for a Docker; for a Podman, this machine's `podman`, reading the files its check reads
/// there over the pool's pinned SSH. Nothing is opened or contacted.
fn pointed(
    docker: &Docker,
    ssh: &Path,
    pool: &str,
    remote: &Remote,
    socket: PathBuf,
    settings_in: &Path,
) -> Result<Docker, Finding> {
    Ok(match &remote.pin.engine {
        super::EnginePin::Podman { .. } => {
            let podman = super::podman_here().ok_or_else(|| Finding::NeedsPodman {
                host: remote.route.host().to_owned(),
            })?;
            docker
                .clone()
                .as_program(podman)
                .through(socket)
                .reaching(super::Reach {
                    ssh: ssh.to_path_buf(),
                    route: remote.route.clone(),
                    pool: pool.to_owned(),
                    key: remote.pin.host_key.clone(),
                    settings_in: settings_in.to_path_buf(),
                })
        }
        super::EnginePin::Docker(_) => docker.clone().through(socket),
    })
}

/// **Pool `name`'s `docker` as its link runs it, without opening its forward** (#6269): the
/// program and the files its check reads as [`docker_of`] gives them, pointed at the forward's
/// socket, which the check before every connection opens. `docker` itself for a pool on this
/// machine, or one that is gone.
pub fn docker_for_link(config: &Path, docker: &Docker, name: &str) -> Result<Docker, String> {
    let pools = crate::runners::pools(config).map_err(|why| why.to_string())?;
    let Some(pool) = pools.iter().find(|pool| pool.name() == name) else {
        return Ok(docker.clone());
    };
    let Some(remote) = pool.remote() else {
        return Ok(docker.clone());
    };
    let said = |finding: Finding| crate::poolsetup::connect_said(&finding);
    let (engine_links, settings_in) = folders_for(docker).map_err(said)?;
    let ssh = super::ssh_here()?;
    let socket = super::socket_of(&engine_links, name)?;
    pointed(docker, &ssh, name, remote, socket, &settings_in).map_err(said)
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

/// Each pool's failing forward, by the pin it was tried with, in this process.
type FailingByPool = std::collections::BTreeMap<String, (crate::sshroute::Pin, Failing)>;
static FAILING: std::sync::Mutex<FailingByPool> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// Pool `pool`'s failing forward, where it was tried with `pin`: a pin changed since is tried at
/// once.
fn failing(pool: &str, pin: &crate::sshroute::Pin) -> Option<Failing> {
    FAILING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(pool)
        .filter(|(tried, _)| tried == pin)
        .map(|(_, failed)| failed.clone())
}

fn note_failing(pool: &str, pin: &crate::sshroute::Pin, failed: Failing) {
    FAILING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(pool.to_owned(), (pin.clone(), failed));
}

fn forget_failing(pool: &str) {
    FAILING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(pool);
}

/// **Every failing forward is tried again at its next ask, and looked at again from the start**:
/// the person pressed Check again in Pool setup.
pub fn look_again() {
    FAILING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
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
/// machine answering, the sign-in with the pinned key, and only where that fails the key it
/// presents against the pin. Where all of those hold, what ssh said.
fn diagnosed(ssh: &Path, remote: &Remote, pool: &str, settings_in: &Path, said: String) -> Finding {
    let route = &remote.route;
    let resolved = match connect::resolve(ssh, route) {
        Ok(resolved) => resolved,
        Err(finding) => return finding,
    };
    if let Err(finding) = connect::reachable(route, &resolved) {
        return finding;
    }
    // Signed in with the pinned key first, and met with nothing signed in only where that fails
    // (`connect::probe`): each meeting is a connection without authentication, which an sshd's
    // per-source penalties count against this machine's address, and a link that keeps
    // reconnecting would look for why again and again.
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

/// **How long the first failed forward waits before it is tried again**: each failure after
/// doubles it, up to [`RETRY_MOST`].
pub const RETRY_FIRST: Duration = Duration::from_secs(5);

/// **The longest a failing forward waits between tries.**
pub const RETRY_MOST: Duration = Duration::from_secs(120);

/// **How long why a forward failed is taken as still why**: within it a failed try is not looked
/// at again from the start ([`diagnosed`] meets the machine several times).
pub const FINDING_KEPT_FOR: Duration = Duration::from_secs(300);

/// **A pool's forward that failed, as this process remembers it**: why, when that was found, how
/// many tries in a row have failed, and when the next may be made. Nothing is contacted before
/// then; Check again in Pool setup forgets it ([`look_again`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failing {
    pub finding: Finding,
    pub found_at: Instant,
    pub failures: u32,
    pub next_try: Instant,
}

/// What a forward's next ask does, from what is remembered of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// Try the forward.
    Try,
    /// Answer with the remembered finding, contacting nothing.
    Answer(Finding),
}

/// **The wait after `failures` failed tries in a row**: [`RETRY_FIRST`], doubled with each one
/// after the first, never more than [`RETRY_MOST`].
#[must_use]
pub fn retry_after(failures: u32) -> Duration {
    let doublings = failures.saturating_sub(1).min(31);
    RETRY_FIRST
        .checked_mul(1u32 << doublings)
        .map_or(RETRY_MOST, |wait| wait.min(RETRY_MOST))
}

impl Failing {
    /// The first failure of a forward, found `now`.
    #[must_use]
    pub fn first(finding: Finding, now: Instant) -> Failing {
        Failing {
            finding,
            found_at: now,
            failures: 1,
            next_try: now + retry_after(1),
        }
    }

    /// **What an ask made `now` does.**
    #[must_use]
    pub fn ask(&self, now: Instant) -> Ask {
        if now < self.next_try {
            Ask::Answer(self.finding.clone())
        } else {
            Ask::Try
        }
    }

    /// **Whether a try that failed `now` is looked at again to say why**, rather than answered
    /// with the finding kept.
    #[must_use]
    pub fn looks_again(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.found_at) >= FINDING_KEPT_FOR
    }

    /// **The same forward failed again `now`**, with `finding` where it was looked at again.
    #[must_use]
    pub fn failed_again(&self, now: Instant, finding: Option<Finding>) -> Failing {
        let failures = self.failures.saturating_add(1);
        let (finding, found_at) = match finding {
            Some(finding) => (finding, now),
            None => (self.finding.clone(), self.found_at),
        };
        Failing {
            finding,
            found_at,
            failures,
            next_try: now + retry_after(failures),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn silent() -> Finding {
        Finding::EngineSilent {
            host: "box".to_owned(),
            said: "no".to_owned(),
        }
    }

    #[test]
    fn a_failing_forward_waits_twice_as_long_each_time_up_to_a_cap() {
        assert_eq!(retry_after(1), RETRY_FIRST);
        assert_eq!(retry_after(2), RETRY_FIRST * 2);
        assert_eq!(retry_after(3), RETRY_FIRST * 4);
        assert_eq!(retry_after(30), RETRY_MOST);
        assert_eq!(retry_after(u32::MAX), RETRY_MOST);
    }

    #[test]
    fn within_its_wait_a_failing_forward_is_answered_with_what_was_found_and_nothing_is_contacted()
    {
        let now = Instant::now();
        let failing = Failing::first(silent(), now);
        assert_eq!(failing.ask(now), Ask::Answer(silent()));
        assert_eq!(
            failing.ask(now + RETRY_FIRST - Duration::from_millis(1)),
            Ask::Answer(silent())
        );
        assert_eq!(failing.ask(now + RETRY_FIRST), Ask::Try);
    }

    #[test]
    fn a_try_that_fails_again_waits_longer_and_keeps_why_for_minutes() {
        let now = Instant::now();
        let failing = Failing::first(silent(), now);
        let later = now + RETRY_FIRST;
        assert!(
            !failing.looks_again(later),
            "why is kept, not looked at again"
        );
        let again = failing.failed_again(later, None);
        assert_eq!(again.failures, 2);
        assert_eq!(again.finding, silent());
        assert_eq!(again.found_at, now);
        assert_eq!(again.next_try, later + RETRY_FIRST * 2);
        assert_eq!(again.ask(later + RETRY_FIRST), Ask::Answer(silent()));
        // Past the minutes why is kept for, a failed try is looked at again, and what it finds
        // is kept from then.
        let much_later = now + FINDING_KEPT_FOR;
        assert!(again.looks_again(much_later));
        let refused = Finding::SignInRefused {
            host: "box".to_owned(),
        };
        let looked = again.failed_again(much_later, Some(refused.clone()));
        assert_eq!(looked.finding, refused);
        assert_eq!(looked.found_at, much_later);
        assert_eq!(looked.failures, 3);
    }
}
