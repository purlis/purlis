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
///
/// **Over Docker's TLS API** (#6267), the same program pointed at the pool's route, its files
/// checked against the pinned CA before every run ([`crate::tlsroute::pool::docker_over`]).
///
/// **What it finds of the pin is kept for the window** ([`crate::poolsetup::pinchange`], #6270): a
/// changed key, CA or engine is found, and a run that gets through says the pin holds.
pub fn docker_of(docker: &Docker, pool: &Pool) -> Result<Docker, Finding> {
    let reached = reach(docker, pool);
    match &reached {
        Ok(_) => crate::poolsetup::pinchange::holds(pool.name()),
        Err(finding) => crate::poolsetup::pinchange::found(pool.name(), finding),
    }
    reached
}

/// **`f`, asked of the engine `engine` names, reached with its pin before any pool is made on
/// it** (#6272, Connect's checks of the engine): over SSH through a forward of its own, opened
/// for this alone and let go of after; over Docker's TLS API at its route. The engine is held to
/// the pin first, so nothing is asked of another.
pub fn with_engine<T>(
    docker: &Docker,
    engine: &crate::runners::RemoteEngine,
    f: impl FnOnce(&Docker) -> T,
) -> Result<T, Finding> {
    let remote = match engine {
        crate::runners::RemoteEngine::Tls(remote) => {
            // One name for every such check: what it keeps is kept by pin.
            let through =
                crate::tlsroute::pool::docker_over(docker, "purlis-connect-check", remote)?;
            return Ok(f(&through));
        }
        crate::runners::RemoteEngine::Ssh(remote) => remote,
    };
    let (engine_links, settings_in) = folders_for(docker)?;
    let ssh = super::ssh_here().map_err(Finding::Here)?;
    let pool = super::connecting_name();
    let opening = Opening {
        ssh: &ssh,
        route: &remote.route,
        pool: &pool,
        key: &remote.pin.host_key,
        engine_links: &engine_links,
        settings_in: &settings_in,
    };
    let held = forward::open(&opening)
        .map_err(|said| diagnosed(&ssh, remote, &pool, &settings_in, said))?;
    let through = pointed(
        docker,
        &ssh,
        &pool,
        remote,
        held.socket().to_path_buf(),
        &settings_in,
    )?;
    connect::engine_holds(&through, &remote.route, &remote.pin)?;
    let answered = f(&through);
    drop(held);
    Ok(answered)
}

/// [`docker_of`], before what it found is kept.
fn reach(docker: &Docker, pool: &Pool) -> Result<Docker, Finding> {
    let remote = match pool.remote_engine() {
        None => return Ok(docker.clone()),
        Some(crate::runners::RemoteEngine::Tls(remote)) => {
            return crate::tlsroute::pool::docker_over(docker, pool.name(), remote);
        }
        Some(crate::runners::RemoteEngine::Ssh(remote)) => remote,
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
    let began = Instant::now();
    let (socket, opened) = match forward::hold_noting(&opening) {
        Ok(held) => {
            if kept.is_some() {
                forget_failing(pool);
            }
            // The link is back: a Try now mark is only for a try still in flight.
            forget_cleared(pool);
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
            // Try now was pressed while this try ran: its failure is not remembered.
            if !cleared_since(pool, began) {
                note_failing(pool, &remote.pin, failed);
            }
            return Err(finding);
        }
    };
    let through = pointed(docker, ssh, pool, remote, socket, settings_in)?;
    if !opened && checked_lately(pool, &remote.pin) {
        return Ok(through);
    }
    let check = |through: &Docker| connect::engine_holds(through, &remote.route, &remote.pin);
    let mut open_again = || {
        // Opened again only once its ssh has ended: one that runs on carries the other links'
        // streams, and an engine that is only slow is not cut from them.
        if !dropped_within(pool, NOTICED_WITHIN) {
            return Ok(None);
        }
        let (socket, _) = forward::hold_noting(&opening)
            .map_err(|said| diagnosed(ssh, remote, pool, settings_in, said))?;
        pointed(docker, ssh, pool, remote, socket, settings_in).map(Some)
    };
    let Through {
        docker: through,
        opened,
        checked,
    } = checked_or_opened_again(through, opened, &check, &mut open_again);
    if let Err(finding) = checked {
        // Nothing more goes over a forward to an engine that isn't the pinned one.
        forget_checked(pool);
        // Asked of a forward just opened only, so at most once an opening: a server that
        // refuses it is let go of below, and its next try opens it again.
        let finding = match finding {
            Finding::EngineSilent { said, .. } if opened => {
                connect::silent_or_refused(&opening, said)
            }
            finding => finding,
        };
        if matches!(
            finding,
            Finding::EngineChanged { .. } | Finding::ForwardingRefused { .. }
        ) {
            forward::let_go(pool);
        }
        if matches!(finding, Finding::ForwardingRefused { .. }) {
            // A forward the server refuses opens here all the same and carries nothing: it
            // waits, and is looked at again, as one that won't open does (#6349).
            let now = Instant::now();
            let failed = match kept {
                Some(kept) => kept.failed_again(now, Some(finding.clone())),
                None => Failing::first(finding.clone(), now),
            };
            note_failing(pool, &remote.pin, failed);
        }
        return Err(finding);
    }
    note_checked(pool, &remote.pin);
    Ok(through)
}

/// How long a forward's `ssh` is given to notice that its far end went away, once a check
/// through it failed: the check's own write is what tells it, and it ends within moments.
const NOTICED_WITHIN: Duration = Duration::from_secs(2);

/// Whether pool `pool`'s forward has dropped, or drops within `within`.
fn dropped_within(pool: &str, within: Duration) -> bool {
    let until = Instant::now() + within;
    loop {
        if forward::dropped(pool) {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// What [`checked_or_opened_again`] ends on: the `docker` pointed at the forward, whether that
/// forward was opened now, and the engine's check through it.
#[derive(Debug, PartialEq)]
pub(crate) struct Through<D> {
    pub docker: D,
    pub opened: bool,
    pub checked: Result<(), Finding>,
}

/// **The engine's check through `held`, opened once again where a forward held from before went
/// silent** (ADR 0123, amended 2026-11-17): after its machine restarts, a forward's `ssh` runs on
/// and its socket stays until something crosses it, so it looks alive while nothing gets through.
/// One held from before (`opened` false) whose engine is silent is handed to `open_again`, which
/// opens it again and answers the new one, or answers `None` where it still holds (its `ssh`
/// runs on: the engine is slow, and the forward is kept for the links on it); the new one is
/// checked once more. One just opened, or an engine that answers as another, is not.
pub(crate) fn checked_or_opened_again<D>(
    held: D,
    opened: bool,
    check: &dyn Fn(&D) -> Result<(), Finding>,
    open_again: &mut dyn FnMut() -> Result<Option<D>, Finding>,
) -> Through<D> {
    match check(&held) {
        Err(silent @ Finding::EngineSilent { .. }) if !opened => match open_again() {
            Ok(Some(again)) => {
                let checked = check(&again);
                Through {
                    docker: again,
                    opened: true,
                    checked,
                }
            }
            Ok(None) => Through {
                docker: held,
                opened,
                checked: Err(silent),
            },
            Err(finding) => Through {
                docker: held,
                opened,
                checked: Err(finding),
            },
        },
        checked => Through {
            docker: held,
            opened,
            checked,
        },
    }
}

/// **`docker`, as pool `pool` on another machine runs it, pointed at `socket`** (#6269): Docker's
/// client for a Docker; for a Podman, this machine's `podman`, reading the files its check reads
/// there over the pool's pinned SSH. Nothing is opened or contacted.
///
/// It keeps what the pin's check reads of the engine through it for the first daemon check made
/// on it after ([`Docker::keeping_what_its_pin_said`], #7862, T6 Q2): a cold link reads the
/// daemon's options once, in the pin's `docker info`.
fn pointed(
    docker: &Docker,
    ssh: &Path,
    pool: &str,
    remote: &Remote,
    socket: PathBuf,
    settings_in: &Path,
) -> Result<Docker, Finding> {
    let docker = &docker.clone().keeping_what_its_pin_said();
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
/// socket, which the check before every connection opens; over TLS (#6267), pointed at its route.
/// `docker` itself for a pool on this machine, or one that is gone.
pub fn docker_for_link(config: &Path, docker: &Docker, name: &str) -> Result<Docker, String> {
    let pools = crate::runners::pools(config).map_err(|why| why.to_string())?;
    let Some(pool) = pools.iter().find(|pool| pool.name() == name) else {
        return Ok(docker.clone());
    };
    // Over Docker's TLS API (#6267), its route, its files checked against the pinned CA as each
    // run is prepared: never this machine's own engine.
    if let Some(crate::runners::RemoteEngine::Tls(remote)) = pool.remote_engine() {
        return Ok(docker
            .clone()
            .over_tls(crate::tlsroute::Through::of(remote)));
    }
    let Some(remote) = pool.remote() else {
        return Ok(docker.clone());
    };
    let said = |finding: Finding| crate::poolsetup::connect_said(&finding);
    let (engine_links, settings_in) = folders_for(docker).map_err(said)?;
    let ssh = super::ssh_here()?;
    let socket = super::socket_of(&engine_links, name)?;
    pointed(docker, &ssh, name, remote, socket, &settings_in).map_err(said)
}

/// **The `docker` a link to a runner of pool `name` is made with**: `docker` itself for a pool on
/// this machine, else [`docker_for_link`]'s. Where a pool on another machine can't be pointed at
/// its engine the link is refused with why: it never falls back to this machine's engine.
///
/// The pools are read first: a record that can't be read refuses the link, since it may name a
/// pool on another machine.
pub fn docker_for_kept_link(config: &Path, docker: &Docker, name: &str) -> Result<Docker, String> {
    let pools = crate::runners::pools(config).map_err(|why| why.to_string())?;
    let elsewhere = pools
        .iter()
        .find(|pool| pool.name() == name)
        .is_some_and(Pool::on_another_machine);
    if !elsewhere {
        return Ok(docker.clone());
    }
    docker_for_link(config, docker, name)
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
    crate::tlsroute::pool::look_again();
    FAILING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
}

/// **Try now** (#7187, item 1): pool `pool`'s failing forward is forgotten, so its next ask
/// opens the forward at once rather than answering what was found until [`Failing::next_try`].
/// The wait of [`retry_after`] is for purlis's own automatic retries; a click is never held to
/// it. Other pools' failing forwards keep their waits.
pub fn try_now(pool: &str) {
    forget_failing(pool);
    CLEARED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(pool.to_owned(), Instant::now());
}

fn forget_cleared(pool: &str) {
    CLEARED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(pool);
}

/// **Pool `pool` is gone from this machine** (removed or forgotten): what this process kept of
/// it, its failing forward, its checked engine and its Try now mark, goes with it.
pub fn pool_gone(pool: &str) {
    forget_failing(pool);
    forget_checked(pool);
    forget_cleared(pool);
}

/// When each pool's Try now was last pressed: a try that began before it does not write its
/// failure back, or the click would be answered from the memory it just cleared.
static CLEARED: std::sync::Mutex<std::collections::BTreeMap<String, Instant>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

fn cleared_since(pool: &str, began: Instant) -> bool {
    CLEARED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(pool)
        .is_some_and(|at| *at >= began)
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
    // Over TLS there is no socket here: a sign-in is bound to the pool's route, the one engine it
    // reaches (#6267).
    if let Some(tls) = through.over_tls_to() {
        let route = PathBuf::from(tls.route.spelled());
        return Ok((through, route));
    }
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
        .is_some_and(|pool| pool.on_another_machine())
}

/// **Where pool `pool`'s forward has its socket on this machine**, whether or not it is open.
pub fn socket_for(docker: &Docker, pool: &str) -> Result<PathBuf, Finding> {
    let (engine_links, _) = folders_for(docker)?;
    super::socket_of(&engine_links, pool).map_err(Finding::Here)
}

/// **[`docker_of`] for the pool named `name`** under `config`, in words: what a link, a check or
/// a make reads before it runs anything there.
pub fn docker_of_named(config: &Path, docker: &Docker, name: &str) -> Result<Docker, String> {
    docker_of_named_in_state(config, docker, name).map_err(|(said, _)| said)
}

/// [`docker_of_named`], with the state a failure puts the pool in (#7187): what a link says of
/// a pool it can't reach, and whether it keeps trying.
pub fn docker_of_named_in_state(
    config: &Path,
    docker: &Docker,
    name: &str,
) -> Result<Docker, (String, connect::RemoteState)> {
    // A runners file that cannot be read is Refused with its reason, not Unreachable: no machine
    // is silent, and waiting does not mend a file that is not as purlis writes it (#7187).
    let pools = crate::runners::pools(config)
        .map_err(|why| (why.to_string(), connect::RemoteState::Refused))?;
    match pools.iter().find(|pool| pool.name() == name) {
        Some(pool) => docker_of(docker, pool).map_err(|finding| {
            (
                crate::poolsetup::connect_said(&finding),
                finding.remote_state(),
            )
        }),
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
mod tests;
