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
    /// The key file it signs in with, where it was picked from a Podman connection (#6269).
    pub identity: Option<String>,
    /// The key the machine showed at the last press of Connect to, in this read only.
    pub met: Option<Met>,
    /// The pin the person trusted, or the pool's.
    pub trusted: Option<Pin>,
    /// What the last contact found wrong.
    pub finding: Option<Finding>,
    /// **Over Docker's TLS API** (#6267): the files named under Options, the CA read at the last
    /// press of Connect to (in this read only), and the CA pin the person trusted, or the pool's.
    pub tls: Option<crate::tlsroute::Files>,
    pub met_ca: Option<CaMet>,
    pub trusted_tls: Option<crate::tlsroute::Pin>,
}

/// **The CA a press of Connect to read** for a machine over TLS (#6267), the machine's own
/// certificate found to be signed by it: what the person checks before trusting it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaMet {
    pub fingerprint: crate::tlsroute::Fingerprint,
    pub subject: Option<String>,
    pub file: String,
}

/// A CA, as Connect shows it (#6267).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CaShown {
    /// Its SHA-256, as `openssl x509 -fingerprint -sha256` prints it.
    pub fingerprint: String,
    pub subject: Option<String>,
    pub file: String,
}

/// The TLS files named, as Connect's Options show them (#6267): paths, and a vault entry's names.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TlsShown {
    pub ca: String,
    pub cert: String,
    pub key: Option<String>,
    pub key_vault: Option<String>,
    pub key_name: Option<String>,
}

impl From<&crate::tlsroute::Files> for TlsShown {
    fn from(files: &crate::tlsroute::Files) -> TlsShown {
        let (key, key_vault, key_name) = match &files.key {
            crate::tlsroute::KeyFrom::File(path) => (Some(path.display().to_string()), None, None),
            crate::tlsroute::KeyFrom::Vault { vault, key, .. } => {
                (None, Some(vault.clone()), Some(key.clone()))
            }
        };
        TlsShown {
            ca: files.ca.display().to_string(),
            cert: files.cert.display().to_string(),
            key,
            key_vault,
            key_name,
        }
    }
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
    /// The key file the route signs in with, where a Podman connection names one (#6269).
    pub identity: Option<String>,
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
    /// **Over Docker's TLS API** (#6267): `"tls"` for a `tcp://` machine, `"ssh"` otherwise.
    pub over: &'static str,
    /// The files named under Options.
    pub tls: Option<TlsShown>,
    /// The CA read at Connect to, to confirm before trusting it.
    pub ca: Option<CaShown>,
    /// The CA's fingerprint trusted for it.
    pub trusted_ca: Option<String>,
    /// The fingerprint the CA file holds now, where that isn't the one trusted.
    pub ca_now: Option<String>,
}

/// The machine's name in a sentence: the host of the route, or "the machine".
fn host_of(route: Option<&str>) -> String {
    route
        .and_then(|route| Route::parse(route).ok())
        .map_or_else(|| "the machine".to_owned(), |route| route.host().to_owned())
}

/// **What Connect's facts mean**: its reading, the machine it shows and the cause of a finding.
pub(super) fn reading(connect: &Connecting) -> (Reading, Machine, Option<&'static str>) {
    if connect
        .route
        .as_deref()
        .is_some_and(crate::tlsroute::is_tcp)
    {
        return reading_tls(connect);
    }
    let host = host_of(connect.route.as_deref());
    let mut machine = Machine {
        route: connect.route.clone(),
        identity: connect.identity.clone(),
        host: connect.route.as_ref().map(|_| host.clone()),
        trusted: connect.trusted.as_ref().map(|pin| (&pin.host_key).into()),
        engine: connect.trusted.as_ref().map(|pin| pin.engine.shown()),
        over: "ssh",
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

/// **[`reading`] for a machine reached at a `tcp://` address** (#6267): refused as plain TCP until
/// its TLS files are named, then read by Connect to and trusted as SSH is.
fn reading_tls(connect: &Connecting) -> (Reading, Machine, Option<&'static str>) {
    let route = connect.route.clone().unwrap_or_default();
    let parsed = crate::tlsroute::Route::parse(&route);
    let host = parsed.as_ref().map_or_else(
        |_| "the machine".to_owned(),
        |route| route.host().to_owned(),
    );
    let mut machine = Machine {
        route: Some(route.clone()),
        host: parsed.as_ref().ok().map(|_| host.clone()),
        over: "tls",
        tls: connect.tls.as_ref().map(TlsShown::from),
        trusted_ca: connect.trusted_tls.as_ref().map(|pin| pin.ca.shown()),
        engine: connect.trusted_tls.as_ref().map(|pin| pin.engine.clone()),
        ..Machine::default()
    };
    if let Some(finding) = &connect.finding {
        if let Finding::CaChanged { now, .. } = finding {
            machine.ca_now = Some(now.clone());
        }
        let (words, next) = said_with_next(finding);
        return (
            Reading::attention(words, next),
            machine,
            Some(cause(finding)),
        );
    }
    if let Err(why) = parsed {
        return (
            Reading::attention(
                why,
                Some(Action {
                    id: ActionId::ChangeMachine,
                    label: "Change the machine".to_owned(),
                    does: "Goes back to the machine, to pick or type it again.".to_owned(),
                    command: None,
                    url: None,
                }),
            ),
            machine,
            Some("not-a-route"),
        );
    }
    if connect.tls.is_none() {
        // Plain tcp:// until the files Docker's --tlsverify takes are named (ADR 0123 §2).
        return (
            Reading::attention(
                format!(
                    "{} If TLS is on for Docker there, name its CA, client certificate and key \
                     under Options.",
                    crate::sshroute::PLAIN_TCP
                ),
                Some(Action {
                    id: ActionId::UseSshInstead,
                    label: "Use SSH instead".to_owned(),
                    does: "Goes back to the machine, to pick or type it again.".to_owned(),
                    command: None,
                    url: None,
                }),
            ),
            machine,
            Some("plain-tcp"),
        );
    }
    if let Some(pin) = &connect.trusted_tls {
        return (
            Reading::done(format!("{host}, CA {}", pin.ca.shown())),
            machine,
            None,
        );
    }
    let Some(met) = &connect.met_ca else {
        return (
            Reading::to_do(
                format!("purlis hasn't contacted {host} yet."),
                Some(Action {
                    id: ActionId::ConnectMachine,
                    label: format!("Connect to {host}"),
                    does: CONNECT_TLS_DOES.to_owned(),
                    command: None,
                    url: None,
                }),
            ),
            machine,
            None,
        );
    };
    machine.ca = Some(CaShown {
        fingerprint: met.fingerprint.shown(),
        subject: met.subject.clone(),
        file: met.file.clone(),
    });
    (
        Reading::to_do(
            format!(
                "{host}'s certificate is signed by this CA. Check it is the CA Docker on {host} \
                 trusts before you trust it: run the command under See what this does there, and \
                 compare the two fingerprints."
            ),
            Some(Action {
                id: ActionId::TrustMachine,
                label: "Trust this machine".to_owned(),
                does: format!(
                    "Keeps this CA for the pool, then checks that Docker answers on {host}. \
                     purlis connects only to a machine whose certificate this CA signed, and \
                     checks the file still holds it before every run."
                ),
                command: None,
                url: None,
            }),
        ),
        machine,
        None,
    )
}

/// What Connect to does over TLS.
const CONNECT_TLS_DOES: &str = "Reads the CA file, and checks that the machine's certificate is \
                                signed by it and accepts yours. It keeps nothing, and nothing on \
                                the machine changes.";

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
        Finding::NeedsPodman { .. } => "needs-podman",
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
        Finding::EngineChanged { host, podman, .. } => (
            if *podman {
                format!(
                    "Podman on {host} isn't the one you trusted: its store is somewhere else now, \
                     or it was reset or reinstalled. purlis makes and links no runner there until \
                     you check it and trust it again. Runners made on the old store aren't on \
                     this one."
                )
            } else {
                format!(
                    "The Docker engine on {host} isn't the one you trusted: it was reset or \
                     reinstalled. purlis makes and links no runner there until you check it and \
                     trust it again. Runners made on the old engine aren't on this one."
                )
            },
            Some(Action {
                id: ActionId::CheckAndTrustAgain,
                label: "Check and trust again".to_owned(),
                does: "Checks the engine there again, for you to trust.".to_owned(),
                command: None,
                url: None,
            }),
        ),
        Finding::NeedsPodman { host } => (
            format!(
                "{host} runs Podman, and purlis reaches Podman with Podman's own client. Install \
                 Podman on {}, then check again.",
                super::this_machine()
            ),
            check_again(),
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
    if let Some(tcp) = route.filter(|route| crate::tlsroute::is_tcp(route)) {
        return by_hand_tls(tcp, None);
    }
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

/// **The route Connect keeps, read**: `route` as it was given, signing in with `identity` where
/// a Podman connection names one (#6269).
pub fn route_of(route: &str, identity: Option<&std::path::Path>) -> Result<Route, Finding> {
    let route = Route::parse(route).map_err(Finding::Here)?;
    match identity {
        Some(identity) => route.with_identity(identity).map_err(Finding::Here),
        None => Ok(route),
    }
}

/// **What a press of Connect to finds for the machine Connect keeps** (#6266, #6267), by its
/// route: the key an SSH machine shows, or the CA a TLS machine's files hold, its certificate
/// found signed by it. Nothing is kept.
pub fn meet_chosen(docker: &crate::dockerpool::Docker, chosen: &super::Chosen) -> super::Pressed {
    if crate::tlsroute::is_tcp(&chosen.route) {
        let met = match &chosen.tls {
            Some(files) => meet_tls(&chosen.route, files),
            None => Err(Finding::Here(crate::sshroute::PLAIN_TCP.to_owned())),
        };
        return match met {
            Ok(met) => super::Pressed::MetCa(met),
            Err(finding) => super::Pressed::Found(finding),
        };
    }
    match chosen.route_read().and_then(|route| meet(docker, &route)) {
        Ok(met) => super::Pressed::Met(met),
        Err(finding) => super::Pressed::Found(finding),
    }
}

/// **What a press of Trust this machine does for the machine Connect keeps**, by its route, the
/// person having confirmed `fingerprint`: the pin kept for the Pool step ([`super::trusted`],
/// [`super::trusted_tls`]), or what went wrong, with nothing kept.
pub fn trust_chosen(
    config: &std::path::Path,
    docker: &crate::dockerpool::Docker,
    chosen: &super::Chosen,
    fingerprint: &str,
) -> Result<Result<(), Finding>, crate::runners::Refused> {
    if crate::tlsroute::is_tcp(&chosen.route) {
        let Some(files) = &chosen.tls else {
            return Ok(Err(Finding::Here(crate::sshroute::PLAIN_TCP.to_owned())));
        };
        return match trust_tls(docker, &chosen.route, files, fingerprint) {
            Ok(remote) => super::trusted_tls(config, &remote).map(Ok),
            Err(finding) => Ok(Err(finding)),
        };
    }
    match chosen
        .route_read()
        .and_then(|route| trust(docker, &route, fingerprint))
    {
        Ok(remote) => super::trusted(config, &remote).map(Ok),
        Err(finding) => Ok(Err(finding)),
    }
}

/// **What Connect to finds over TLS** (#6267): the CA `files` name, read, and the machine at
/// `route` met by purlis's own handshake against it with the client certificate. Nothing is kept.
pub fn meet_tls(route: &str, files: &crate::tlsroute::Files) -> Result<CaMet, Finding> {
    let route = crate::tlsroute::Route::parse(route).map_err(Finding::Here)?;
    crate::tlsroute::held_from_chats(files, &crate::tlsroute::folders_chats_write(), route.host())?;
    let ca = crate::tlsroute::read_ca(&files.ca, route.host())?;
    let through = crate::tlsroute::Through {
        route: route.clone(),
        files: files.clone(),
        ca: ca.fingerprint,
        engine: None,
    };
    let material = through.material()?;
    crate::tlsroute::handshake(&route, files, &material)?;
    Ok(CaMet {
        fingerprint: ca.fingerprint,
        subject: ca.subject(),
        file: files.ca.display().to_string(),
    })
}

/// **What Trust this machine does over TLS** (#6267, ADR 0123 §3): the CA read again and held to
/// `fingerprint`, the one the person confirmed, the machine met again, and the engine's id read
/// over TLS. The route, its files and the pin. Nothing is kept here.
pub fn trust_tls(
    docker: &crate::dockerpool::Docker,
    route: &str,
    files: &crate::tlsroute::Files,
    fingerprint: &str,
) -> Result<crate::tlsroute::Remote, Finding> {
    let route = crate::tlsroute::Route::parse(route).map_err(Finding::Here)?;
    let confirmed = crate::tlsroute::Fingerprint::parse(fingerprint).map_err(Finding::Here)?;
    crate::tlsroute::held_from_chats(files, &crate::tlsroute::folders_chats_write(), route.host())?;
    let ca = crate::tlsroute::read_ca(&files.ca, route.host())?;
    if ca.fingerprint != confirmed {
        // What was confirmed isn't what the file holds now: confirmed again, never trusted unseen.
        return Err(Finding::Here(format!(
            "{} holds the CA {}, not {}, which you confirmed. Connect to {} again and check the \
             CA it holds now.",
            files.ca.display(),
            ca.fingerprint.shown(),
            confirmed.shown(),
            route.host()
        )));
    }
    let through = crate::tlsroute::Through {
        route: route.clone(),
        files: files.clone(),
        ca: ca.fingerprint,
        engine: None,
    };
    let material = through.material()?;
    crate::tlsroute::handshake(&route, files, &material)?;
    let engine = docker
        .clone()
        .over_tls(through)
        .engine_id_said()
        .map_err(|why| Finding::EngineSilent {
            host: route.host().to_owned(),
            said: why.0,
        })?;
    Ok(crate::tlsroute::Remote {
        route,
        files: files.clone(),
        pin: crate::tlsroute::Pin {
            ca: ca.fingerprint,
            engine,
        },
    })
}

/// **[`by_hand`] over TLS** (#6267): what prints the CA's fingerprint on the machine, for the
/// person to compare, and the engine check with the files named, as purlis runs it.
#[must_use]
pub fn by_hand_tls(route: &str, files: Option<&crate::tlsroute::Files>) -> super::See {
    let parsed = crate::tlsroute::Route::parse(route).ok();
    let host = parsed
        .as_ref()
        .map_or_else(|| "the machine".to_owned(), |route| route.host().to_owned());
    let address = parsed.map_or_else(|| "tcp://HOST:2376".to_owned(), |route| route.spelled());
    let q = |text: &str| crate::shellseg::shell_quote(text);
    let (ca, cert, key) = files.map_or_else(
        || {
            (
                "ca.pem".to_owned(),
                "cert.pem".to_owned(),
                "key.pem".to_owned(),
            )
        },
        |files| {
            (
                files.ca.display().to_string(),
                files.cert.display().to_string(),
                files
                    .key_file()
                    .map_or_else(|| "key.pem".to_owned(), |key| key.display().to_string()),
            )
        },
    );
    super::See {
        note: format!(
            "The first command, run on {host} itself with the CA Docker's --tlscacert names \
             there, prints its fingerprint, to compare with the one shown here. The second asks \
             Docker there for its engine's id, as purlis does. Both only read."
        ),
        text: format!(
            "openssl x509 -noout -fingerprint -sha256 -in /etc/docker/ca.pem\n\
             docker -H {} --tlsverify --tlscacert {} --tlscert {} --tlskey {} info --format \
             '{{{{.ID}}}}'",
            q(&address),
            q(&ca),
            q(&cert),
            q(&key)
        ),
    }
}

/// **What Connect to finds**: the key the machine at `route` shows, met with nothing signed in,
/// and the one the person's own ssh knows for it. Nothing is kept.
pub fn meet(docker: &crate::dockerpool::Docker, route: &Route) -> Result<Met, Finding> {
    let ssh = crate::sshroute::ssh_here().map_err(Finding::Here)?;
    let (_, settings_in) = crate::sshroute::pool::folders_for(docker)?;
    let resolved = crate::sshroute::connect::resolve(&ssh, route)?;
    crate::sshroute::connect::reachable(route, &resolved)?;
    let key = crate::sshroute::connect::meet(&ssh, route, &settings_in)?;
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
    route: &Route,
    fingerprint: &str,
) -> Result<Remote, Finding> {
    let ssh = crate::sshroute::ssh_here().map_err(Finding::Here)?;
    let (engine_links, settings_in) = crate::sshroute::pool::folders_for(docker)?;
    let resolved = crate::sshroute::connect::resolve(&ssh, route)?;
    crate::sshroute::connect::reachable(route, &resolved)?;
    let key = crate::sshroute::connect::meet(&ssh, route, &settings_in)?;
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
        route,
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
    let engine = crate::sshroute::connect::engine_pin(docker, &opening)?;
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
