// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Connect, the step of a pool on another machine** (#6266, ADR 0123 §3, §6 and §7). Its facts
//! ([`Connecting`]), what they mean ([`reading`]), the words of each finding ([`said`]), and the
//! two presses that contact the machine: [`meet`], which only reads the key it shows, and
//! [`trust`], which pins that key, finds the engine's socket and reads the engine's id.
//!
//! Nothing contacts the machine but those presses, and nothing is kept but what the person
//! trusted ([`super::Choices::connect`]).

use serde::Serialize;

use super::{Action, ActionId, Reading};
use crate::runners::Remote;
use crate::sshroute::connect::Finding;
use crate::sshroute::{HostKey, Pin, Route};

/// **What Connect knows**, before it means anything.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Connecting {
    /// The machine picked or typed: Docker's spelling, as the person gave it.
    pub route: Option<String>,
    /// The key the machine showed at the last press of Connect to, in this read only.
    pub met: Option<Met>,
    /// The pin the person trusted, or the pool's.
    pub trusted: Option<Pin>,
    /// What the last contact found wrong.
    pub finding: Option<Finding>,
}

/// **The key a first contact met**, and the one the person's own ssh knows for the machine,
/// where it knows one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Met {
    pub key: HostKey,
    pub known: Option<HostKey>,
}

/// A key, as Connect shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyShown {
    pub algorithm: String,
    pub fingerprint: String,
}

impl From<&HostKey> for KeyShown {
    fn from(key: &HostKey) -> KeyShown {
        KeyShown {
            algorithm: key.algorithm().to_owned(),
            fingerprint: key.fingerprint(),
        }
    }
}

/// **The machine, as Connect shows it**: what the window draws its facts card and its key from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Machine {
    pub route: Option<String>,
    pub host: Option<String>,
    /// The key it showed, to confirm before trusting it.
    pub key: Option<KeyShown>,
    /// Whether the person's own ssh already knows that key for it: it does, it knows another,
    /// or it knows none (`None`).
    pub known: Option<bool>,
    /// The key trusted for it.
    pub trusted: Option<KeyShown>,
    /// The key it shows now, where that isn't the one trusted.
    pub now: Option<KeyShown>,
    /// The engine's id trusted for it.
    pub engine: Option<String>,
}

/// The machine's name in a sentence: the host of the route, or "the machine".
fn host_of(route: Option<&str>) -> String {
    route
        .and_then(|route| Route::parse(route).ok())
        .map_or_else(|| "the machine".to_owned(), |route| route.host().to_owned())
}

/// **What Connect's facts mean**: its reading, the machine it shows and the cause of a finding.
pub(super) fn reading(connect: &Connecting) -> (Reading, Machine, Option<&'static str>) {
    let host = host_of(connect.route.as_deref());
    let mut machine = Machine {
        route: connect.route.clone(),
        host: connect.route.as_ref().map(|_| host.clone()),
        trusted: connect.trusted.as_ref().map(|pin| (&pin.host_key).into()),
        engine: connect.trusted.as_ref().map(|pin| pin.engine.clone()),
        ..Machine::default()
    };
    if let Some(finding) = &connect.finding {
        if let Finding::KeyChanged { now, .. } = finding {
            machine.now = Some(now.into());
        }
        let (words, next) = said_with_next(finding);
        return (
            Reading::attention(words, next),
            machine,
            Some(cause(finding)),
        );
    }
    if let Some(pin) = &connect.trusted {
        return (
            Reading::done(format!("{host}, key {}", pin.host_key.fingerprint())),
            machine,
            None,
        );
    }
    let Some(route) = &connect.route else {
        return (
            Reading::to_do(
                "Pick the machine Docker runs on. purlis reaches it with SSH, using your own SSH \
                 settings and agent, and opens no port there.",
                Some(Action {
                    id: ActionId::ConnectMachine,
                    label: "Connect".to_owned(),
                    does: CONNECT_DOES.to_owned(),
                    command: None,
                    url: None,
                }),
            ),
            machine,
            None,
        );
    };
    if let Err(why) = Route::parse(route) {
        let plain_tcp = why == crate::sshroute::PLAIN_TCP;
        return (
            Reading::attention(
                why,
                Some(Action {
                    id: if plain_tcp {
                        ActionId::UseSshInstead
                    } else {
                        ActionId::ChangeMachine
                    },
                    label: if plain_tcp {
                        "Use SSH instead"
                    } else {
                        "Change the machine"
                    }
                    .to_owned(),
                    does: "Goes back to the machine, to pick or type it again.".to_owned(),
                    command: None,
                    url: None,
                }),
            ),
            machine,
            Some(if plain_tcp {
                "plain-tcp"
            } else {
                "not-a-route"
            }),
        );
    }
    let Some(met) = &connect.met else {
        return (
            Reading::to_do(
                format!("purlis hasn't contacted {host} yet."),
                Some(Action {
                    id: ActionId::ConnectMachine,
                    label: format!("Connect to {host}"),
                    does: CONNECT_DOES.to_owned(),
                    command: None,
                    url: None,
                }),
            ),
            machine,
            None,
        );
    };
    machine.key = Some((&met.key).into());
    machine.known = met.known.as_ref().map(|known| *known == met.key);
    let finding = match machine.known {
        Some(true) => format!(
            "This is the key {host} shows, and your SSH already knows it for {host}. Trust it to \
             use this machine for the pool."
        ),
        Some(false) => format!(
            "This is the key {host} shows, and your SSH knows a different one for {host}. Check \
             it on {host} before you trust it: this happens when a machine is reinstalled, and \
             can mean something else is answering in its place."
        ),
        None => format!(
            "This is the key {host} shows. Check it on {host} before you trust it: run the \
             command under See what this does there, and compare the two."
        ),
    };
    (
        Reading::to_do(
            finding,
            Some(Action {
                id: ActionId::TrustMachine,
                label: "Trust this machine".to_owned(),
                does: format!(
                    "Keeps this key for the pool, then checks that Docker answers on {host}. \
                     purlis connects only to a machine showing this key."
                ),
                command: None,
                url: None,
            }),
        ),
        machine,
        None,
    )
}

/// What Connect to does.
const CONNECT_DOES: &str = "Reads the key the machine shows, and keeps nothing. Nothing on it \
                            changes.";

/// **The cause of a finding**, the key the window draws its card by.
#[must_use]
pub fn cause(finding: &Finding) -> &'static str {
    match finding {
        Finding::NoSuchName { .. } => "no-such-name",
        Finding::NoAnswer { .. } => "no-answer",
        Finding::NotMet { .. } => "not-met",
        Finding::KeyChanged { .. } => "pool-pin-changed",
        Finding::SignInRefused { .. } => "sign-in-refused",
        Finding::NoEngine { .. } => "no-engine",
        Finding::NotAllowed { .. } => "not-allowed",
        Finding::EngineSilent { .. } => "engine-silent",
        Finding::EngineChanged { .. } => "pool-engine-changed",
        Finding::TlsFile { .. } => "tls-file",
        Finding::CaChanged { .. } => "pool-pin-changed",
        Finding::NotSignedByCa { .. } => "tls-not-signed",
        Finding::ServerExpired { .. } => "tls-expired",
        Finding::WrongName { .. } => "tls-wrong-name",
        Finding::ClientRefused { .. } => "tls-client-refused",
        Finding::ClientExpired { .. } => "tls-client-expired",
        Finding::Here(_) => "here",
    }
}

/// **What a finding says** (ADR 0123 §7's words), the same in the window and in `purlis pool`.
#[must_use]
pub fn said(finding: &Finding) -> String {
    said_with_next(finding).0
}

fn said_with_next(finding: &Finding) -> (String, Option<Action>) {
    let again = |label: &str, does: &str| {
        Some(Action {
            id: ActionId::CheckAgain,
            label: label.to_owned(),
            does: does.to_owned(),
            command: None,
            url: None,
        })
    };
    let check_again = || {
        again(
            "Check again",
            "Contacts the machine again. It changes nothing.",
        )
    };
    let change = || {
        Some(Action {
            id: ActionId::ChangeMachine,
            label: "Change the machine".to_owned(),
            does: "Goes back to the machine, to pick or type it again.".to_owned(),
            command: None,
            url: None,
        })
    };
    match finding {
        Finding::NoSuchName { host } => (
            format!(
                "purlis couldn't find a machine called {host}. Check the name, or pick it from \
                 your SSH hosts."
            ),
            change(),
        ),
        Finding::NoAnswer { host, port } => (
            format!(
                "{host} didn't answer on port {port}. Check that it's on and on this network, and \
                 connect your VPN if you use one."
            ),
            check_again(),
        ),
        Finding::NotMet { host, said } => (
            format!(
                "purlis couldn't reach SSH on {host}{}. Check that SSH is running there, then \
                 check again.",
                shown(said)
            ),
            check_again(),
        ),
        Finding::KeyChanged {
            host, pinned, now, ..
        } => (
            format!(
                "{host} shows a different key than the one you trusted ({} now, {} trusted). \
                 This happens when the machine is reinstalled, and can mean something else is \
                 answering in its place, so purlis connects to nothing there until you check it \
                 and trust it again.",
                now.fingerprint(),
                pinned.fingerprint()
            ),
            Some(Action {
                id: ActionId::CheckAndTrustAgain,
                label: "Check and trust again".to_owned(),
                does: "Shows the key the machine shows now, for you to check and trust.".to_owned(),
                command: None,
                url: None,
            }),
        ),
        Finding::SignInRefused { host } => (
            format!(
                "{host} didn't accept your sign-in. purlis signs in with your SSH agent and never \
                 asks for a password. Add your key to the agent with `ssh-add`, then check again."
            ),
            check_again(),
        ),
        Finding::NoEngine { host, user } => (
            format!(
                "{host} has no Docker running that {} can use. Start Docker there, or name its \
                 socket in the address, then check again.",
                user_or_you(user)
            ),
            check_again(),
        ),
        Finding::NotAllowed { host, user, .. } => (
            format!(
                "{} on {host} isn't allowed to use Docker. On {host}, add {} to the docker group, \
                 or use rootless Docker or Podman, then check again.",
                user_or_you(user),
                user_or_you(user)
            ),
            check_again(),
        ),
        Finding::EngineSilent { host, said } => (
            format!(
                "Docker on {host} didn't answer through SSH{}. Check that Docker is running \
                 there, then check again.",
                shown(said)
            ),
            check_again(),
        ),
        Finding::EngineChanged { host, .. } => (
            format!(
                "The Docker engine on {host} isn't the one you trusted: it was reset or \
                 reinstalled. purlis makes and links no runner there until you check it and \
                 trust it again. Runners made on the old engine aren't on this one."
            ),
            Some(Action {
                id: ActionId::CheckAndTrustAgain,
                label: "Check and trust again".to_owned(),
                does: "Checks the engine there again, for you to trust.".to_owned(),
                command: None,
                url: None,
            }),
        ),
        Finding::TlsFile { file, why, .. } => (
            format!("{file} {why}. Check the file named under Options, then check again."),
            check_again(),
        ),
        Finding::CaChanged {
            host,
            pinned,
            now,
            file,
        } => (
            format!(
                "{file} no longer holds the CA you trusted for {host} ({now} now, {pinned} \
                 trusted). This happens when the CA is replaced, and can mean something else is \
                 answering in its place, so purlis connects to nothing there until you check it \
                 and trust it again."
            ),
            Some(Action {
                id: ActionId::CheckAndTrustAgain,
                label: "Check and trust again".to_owned(),
                does: "Shows the CA the file holds now, for you to check and trust.".to_owned(),
                command: None,
                url: None,
            }),
        ),
        Finding::NotSignedByCa { host, ca } => (
            format!(
                "{host}'s certificate isn't signed by the CA you chose. Check {ca}: it must be \
                 the CA Docker's certificates on {host} are signed with."
            ),
            check_again(),
        ),
        Finding::ServerExpired { host } => (
            format!(
                "{host}'s certificate has expired. Make Docker a new one on {host}, the file its \
                 --tlscert names there, then check again."
            ),
            check_again(),
        ),
        Finding::WrongName { host, name } => (
            if name.is_empty() {
                format!(
                    "{host}'s certificate is for another name. Connect by the name it is for, or \
                     check the file Docker's --tlscert names on {host}."
                )
            } else {
                format!(
                    "{host}'s certificate is for another name ({name}). Connect by that name, or \
                     check the file Docker's --tlscert names on {host}."
                )
            },
            change(),
        ),
        Finding::ClientRefused { host, cert } => (
            format!(
                "Docker on {host} didn't accept your client certificate. Check {cert}: it must \
                 be signed by the CA Docker on {host} trusts, the file its --tlscacert names."
            ),
            check_again(),
        ),
        Finding::ClientExpired { cert, .. } => (
            format!(
                "Your client certificate has expired. Check {cert}, make a new one, then check again."
            ),
            check_again(),
        ),
        Finding::Here(why) => (why.clone(), check_again()),
    }
}

fn user_or_you(user: &str) -> String {
    if user.is_empty() {
        "your account".to_owned()
    } else {
        user.to_owned()
    }
}

fn shown(said: &str) -> String {
    if said.trim().is_empty() {
        String::new()
    } else {
        format!(" (it said: {})", crate::shown::short(said.trim()))
    }
}

/// **The commands behind Connect's See what this does**: what prints the key on the machine for
/// the person to compare, and the engine check, by hand.
#[must_use]
pub fn by_hand(route: Option<&str>) -> super::See {
    let route = route.and_then(|route| Route::parse(route).ok());
    let host = route
        .as_ref()
        .map_or_else(|| "the machine".to_owned(), |route| route.host().to_owned());
    let address = route.map_or_else(|| "ssh://USER@HOST".to_owned(), |route| route.spelled());
    super::See {
        note: format!(
            "The first command, run on {host} itself, prints its key, to compare with the one \
             shown here. The second asks Docker there for its engine's id, as purlis does. Both \
             only read."
        ),
        text: format!(
            "ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub\n\
             docker -H {} info --format '{{{{.ID}}}}'",
            crate::shellseg::shell_quote(&address)
        ),
    }
}

/// **What Connect to finds**: the key the machine at `route` shows, met with nothing signed in,
/// and the one the person's own ssh knows for it. Nothing is kept.
pub fn meet(docker: &crate::dockerpool::Docker, route: &str) -> Result<Met, Finding> {
    let route = Route::parse(route).map_err(Finding::Here)?;
    let ssh = crate::sshroute::ssh_here().map_err(Finding::Here)?;
    let (_, settings_in) = crate::sshroute::pool::folders_for(docker)?;
    let resolved = crate::sshroute::connect::resolve(&ssh, &route)?;
    crate::sshroute::connect::reachable(&route, &resolved)?;
    let key = crate::sshroute::connect::meet(&ssh, &route, &settings_in)?;
    let home = crate::envvar::var_os("HOME").map(std::path::PathBuf::from);
    let known = crate::sshroute::connect::known_to_the_person(&resolved, home.as_deref());
    Ok(Met { key, known })
}

/// **What Trust this machine does**: the machine at `route` met again, its key held to
/// `fingerprint` (the one the person confirmed), the sign-in checked, the engine's socket found,
/// and the engine's id read through a forward opened against that key. The route, with its
/// socket, and the pin. Nothing is kept here: [`super::trusted`] keeps it.
pub fn trust(
    docker: &crate::dockerpool::Docker,
    route: &str,
    fingerprint: &str,
) -> Result<Remote, Finding> {
    let route = Route::parse(route).map_err(Finding::Here)?;
    let ssh = crate::sshroute::ssh_here().map_err(Finding::Here)?;
    let (engine_links, settings_in) = crate::sshroute::pool::folders_for(docker)?;
    let resolved = crate::sshroute::connect::resolve(&ssh, &route)?;
    crate::sshroute::connect::reachable(&route, &resolved)?;
    let key = crate::sshroute::connect::meet(&ssh, &route, &settings_in)?;
    if key.fingerprint() != fingerprint.trim() {
        // What was confirmed isn't what shows now: confirmed again, never trusted unseen.
        return Err(Finding::Here(format!(
            "{} shows the key {}, not {}, which you confirmed. Connect to it again and check the \
             key it shows now.",
            route.host(),
            key.fingerprint(),
            crate::shown::short(fingerprint.trim())
        )));
    }
    let connecting = crate::sshroute::connecting_name();
    let found = crate::sshroute::connect::probe(
        &ssh,
        &route,
        &connecting,
        &key,
        &settings_in,
        &resolved.user,
    )?;
    let opening = crate::sshroute::forward::Opening {
        ssh: &ssh,
        route: &found,
        pool: &connecting,
        key: &key,
        engine_links: &engine_links,
        settings_in: &settings_in,
    };
    let engine = crate::sshroute::connect::engine_id(docker, &opening)?;
    Ok(Remote {
        route: found,
        pin: Pin {
            host_key: key,
            engine,
        },
    })
}

#[cfg(test)]
mod tests;
