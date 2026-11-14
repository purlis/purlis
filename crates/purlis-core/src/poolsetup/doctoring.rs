// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Doctor, for a pool on another machine** (#6272, ADR 0123 §7): every check of the record's
//! table, read fresh, each failing as one [`Finding`] that Pool setup's Connect step says
//! ([`super::connecting`]) and `purlis pool doctor` prints in the same words ([`said_for`]), with
//! its one way on and that way's commands under See what this does.
//!
//! [`check`] reaches the machine as a run for the pool does ([`crate::sshroute::pool::docker_of`]:
//! the name, the port, the sign-in, the key, the socket, the account, TLS and the engine's id),
//! then asks the engine what the local kind asks of its own ([`engine`]): that it confines
//! runners, that its platform has a runner image, that the image is there or can be downloaded,
//! and that its disk has room. Then the Docker context the route was read from, where it was
//! read from one, and last the link's round trip ([`round_trip`]), which Try it shows too.
//!
//! Every answer is read from what Docker prints as data (`docker info`'s fields, an exit status,
//! `df`'s fourth column), never from its words.

use std::time::{Duration, Instant};

use crate::dockerpool::{Docker, Image};
use crate::sshroute::connect::Finding;

/// **The platforms the runner image is built for** (`.github/workflows/runner-image.yml`, which
/// checks its index holds these and no others).
pub const RUNNER_PLATFORMS: [&str; 2] = ["linux/amd64", "linux/arm64"];

/// How many round trips are timed: the middle one is kept, so one slow answer says nothing.
const ROUND_TRIPS: usize = 3;

/// **A pool on another machine that passed every check**: its machine, and the middle of
/// [`ROUND_TRIPS`] round trips to its engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Healthy {
    pub host: String,
    pub round_trip: Duration,
}

/// **Every check of ADR 0123 §7 on pool `pool`**, with `docker` this machine's: what failed first,
/// or the machine and its round trip. A pool on this machine is refused: it has no other machine.
pub fn check(docker: &Docker, pool: &crate::runners::Pool) -> Result<Healthy, Finding> {
    let host = host_of(pool).ok_or_else(|| {
        Finding::Here(format!(
            "Pool {} runs on this machine's Docker, so it has no other machine to check.",
            pool.name()
        ))
    })?;
    let through = crate::sshroute::pool::docker_of(docker, pool)?;
    engine(&through, &host, pool.image())?;
    if let (Some(name), Some(route)) = (pool.docker_context(), route_of(pool)) {
        let contexts = crate::dockercontext::contexts_here(docker.program());
        if crate::dockercontext::moved(&contexts, name, &route).is_some() {
            return Err(Finding::ContextMoved {
                name: name.to_owned(),
            });
        }
    }
    let round_trip = round_trip(&through).map_err(|why| Finding::EngineSilent {
        host: host.clone(),
        said: why.0,
    })?;
    Ok(Healthy { host, round_trip })
}

/// **What the engine `through` reaches, on machine `host`, is asked for pool image `image`**: that
/// it gives containers its default seccomp profile (the local kind's daemon check), that its
/// platform has a runner image, that the image is there or downloads, that the image there is
/// built for that platform, and that its disk has room for a runner. The first that fails.
///
/// The image is downloaded where the machine lacks it, as making the pool's runner would: that is
/// all this changes there.
pub fn engine(through: &Docker, host: &str, image: &Image) -> Result<(), Finding> {
    let silent = |why: crate::dockerpool::Refused| Finding::EngineSilent {
        host: host.to_owned(),
        said: why.0,
    };
    if through.confines_runners().is_err() {
        return Err(Finding::Unconfined {
            host: host.to_owned(),
            podman: through.is_podman_client(),
        });
    }
    let platform = through.platform().map_err(silent)?;
    let unbuilt = |built_for: Vec<String>| Finding::Architecture {
        host: host.to_owned(),
        platform: platform.clone(),
        built_for,
    };
    let there = match through.image_platform(image).map_err(silent)? {
        Some(there) => there,
        None => {
            if !runs_the_runner_image(&platform) && image.is_purlis_own() {
                return Err(unbuilt(owned(&RUNNER_PLATFORMS)));
            }
            if through.pull(image).is_err() {
                return Err(Finding::ImageUnreachable {
                    host: host.to_owned(),
                    registry: registry_of(image.as_str()),
                });
            }
            through
                .image_platform(image)
                .map_err(silent)?
                .ok_or_else(|| Finding::ImageUnreachable {
                    host: host.to_owned(),
                    registry: registry_of(image.as_str()),
                })?
        }
    };
    if there != platform {
        return Err(unbuilt(vec![there]));
    }
    match through.room_for(image) {
        crate::dockerpool::Room::Short(free) => Err(Finding::DiskFull {
            host: host.to_owned(),
            free,
        }),
        crate::dockerpool::Room::FullSaid(_) => Err(Finding::DiskFull {
            host: host.to_owned(),
            free: None,
        }),
        crate::dockerpool::Room::Enough | crate::dockerpool::Room::Unknown => Ok(()),
    }
}

/// **The link's round trip** (ADR 0123 §6, Try it): the middle of [`ROUND_TRIPS`] asks of the
/// engine, each one request over the same connection the link rides.
pub fn round_trip(through: &Docker) -> Result<Duration, crate::dockerpool::Refused> {
    let mut took = Vec::with_capacity(ROUND_TRIPS);
    for _ in 0..ROUND_TRIPS {
        let started = Instant::now();
        through.answers()?;
        took.push(started.elapsed());
    }
    Ok(middle(&mut took))
}

/// The middle of `took`, once sorted.
#[must_use]
pub fn middle(took: &mut [Duration]) -> Duration {
    took.sort_unstable();
    took.get(took.len() / 2).copied().unwrap_or_default()
}

/// **A round trip, said**: whole milliseconds, never less than one.
#[must_use]
pub fn round_trip_said(host: &str, took: Duration) -> String {
    let ms = took.as_millis().max(1);
    format!("A round trip to {host} takes {ms} ms.")
}

/// **Try it's line for the link to `host`** (ADR 0123 §6): its round trip `measured`, or why it
/// couldn't be, in Pool setup's words.
#[must_use]
pub fn link_check(host: &str, measured: &Result<Duration, Finding>) -> super::Check {
    match measured {
        Ok(took) => super::Check {
            about: super::CheckOf::Link,
            verdict: super::Verdict::Ok,
            title: round_trip_said(host, *took),
            more: None,
        },
        Err(finding) => super::Check {
            about: super::CheckOf::Link,
            verdict: super::Verdict::Missing,
            title: format!("purlis couldn't measure the round trip to {host}"),
            more: Some(super::connecting::said(finding)),
        },
    }
}

/// **Try it's line for pool `pool`'s link**, where it is on another machine: reached as a run
/// for it is, and its round trip measured. `None` for a pool on this machine.
#[must_use]
pub fn link_of(docker: &Docker, pool: &crate::runners::Pool) -> Option<super::Check> {
    let host = host_of(pool)?;
    let measured = crate::sshroute::pool::docker_of(docker, pool).and_then(|through| {
        round_trip(&through).map_err(|why| Finding::EngineSilent {
            host: host.clone(),
            said: why.0,
        })
    });
    Some(link_check(&host, &measured))
}

/// **What `purlis pool doctor` says of pool `pool`, whose machine `machine` names, and its exit**: a pool
/// that passed every check, with its round trip, exits 0. A finding is said as Pool setup says
/// it, a changed pin in its Notice's words, then its way on and that way's commands, the same
/// lines the window draws; it exits 1.
#[must_use]
pub fn said_for(
    pool: &str,
    machine: &super::connecting::Connecting,
    found: &Result<Healthy, Finding>,
) -> (String, u8) {
    let finding = match found {
        Ok(healthy) => {
            return (
                format!(
                    "Pool {pool}: {host} is the machine you trusted, and its engine the one you \
                     trusted. Docker there confines runners, has the runner image and room for a \
                     runner.\n{}\n",
                    round_trip_said(&healthy.host, healthy.round_trip),
                    host = healthy.host,
                ),
                0,
            );
        }
        Err(finding) => finding,
    };
    let connect = super::connecting::Connecting {
        finding: Some(finding.clone()),
        ..machine.clone()
    };
    let (words, next) = super::connecting::said_and_next(finding);
    let mut out = match super::pinchange::of(pool, finding) {
        Some(change) => format!("{}\n{}\n", change.title, change.said),
        None => format!("Pool {pool}: {words}\n"),
    };
    if let Some(next) = next {
        out.push_str(&format!("Next: {}. {}\n", next.label, next.does));
    }
    let see = super::connecting::see(&connect);
    out.push_str(&format!("See what this does: {}\n", see.note));
    for line in see.text.lines() {
        out.push_str(&format!("  {line}\n"));
    }
    (out, 1)
}

/// **A platform, as Docker names one** (`linux/amd64`), from what `docker info` says of the
/// engine's `OSType` and `Architecture`, which is the kernel's own word (`x86_64`, `aarch64`).
#[must_use]
pub fn platform_of(os: &str, arch: &str) -> String {
    let arch = match arch.trim() {
        "x86_64" | "x86-64" | "amd64" => "amd64",
        "aarch64" | "arm64" | "armv8" => "arm64",
        other => other,
    };
    format!("{}/{arch}", os.trim().to_ascii_lowercase())
}

/// Whether the runner image is built for `platform`.
#[must_use]
pub fn runs_the_runner_image(platform: &str) -> bool {
    RUNNER_PLATFORMS.contains(&platform)
}

/// **The registry an image is downloaded from**: its name's first part, where that names a host
/// (it has a dot or a port, or is `localhost`), else Docker Hub, as Docker reads a name.
#[must_use]
pub fn registry_of(image: &str) -> String {
    let name = image.split('@').next().unwrap_or(image);
    match name.split_once('/') {
        Some((first, _)) if first.contains('.') || first.contains(':') || first == "localhost" => {
            first.to_owned()
        }
        _ => "docker.io".to_owned(),
    }
}

/// **`items` in a sentence**: `a`, `a and b`, `a, b and c`.
#[must_use]
pub fn and_listed(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn owned(items: &[&str]) -> Vec<String> {
    items.iter().map(|one| (*one).to_owned()).collect()
}

/// The machine of a pool on another machine, `None` for this machine's.
fn host_of(pool: &crate::runners::Pool) -> Option<String> {
    Some(match pool.remote_engine()? {
        crate::runners::RemoteEngine::Ssh(remote) => remote.route.host().to_owned(),
        crate::runners::RemoteEngine::Tls(remote) => remote.route.host().to_owned(),
    })
}

/// The route of a pool on another machine, in Docker's spelling.
fn route_of(pool: &crate::runners::Pool) -> Option<String> {
    Some(match pool.remote_engine()? {
        crate::runners::RemoteEngine::Ssh(remote) => remote.route.spelled(),
        crate::runners::RemoteEngine::Tls(remote) => remote.route.spelled(),
    })
}

/// **Pool `pool`'s machine, as Connect knows it**: its route, and the TLS files it names, for
/// what is said of it. `None` for a pool on this machine.
#[must_use]
pub fn machine_of(pool: &crate::runners::Pool) -> Option<super::connecting::Connecting> {
    Some(match pool.remote_engine()? {
        crate::runners::RemoteEngine::Ssh(remote) => super::connecting::Connecting {
            route: Some(remote.route.spelled()),
            identity: remote
                .route
                .identity()
                .map(|path| path.display().to_string()),
            trusted: Some(remote.pin.clone()),
            ..super::connecting::Connecting::default()
        },
        crate::runners::RemoteEngine::Tls(remote) => super::connecting::Connecting {
            route: Some(remote.route.spelled()),
            tls: Some(remote.files.clone()),
            trusted_tls: Some(remote.pin.clone()),
            ..super::connecting::Connecting::default()
        },
    })
}

#[cfg(test)]
mod tests;
