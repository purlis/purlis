// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::super::connecting::Connecting;
use super::super::*;
use super::*;
use crate::sshroute::HostKey;

const KEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINiGzfjDDJsgYLYCmdkf8Fel9sE6Ma14Y2+c33Y/qZ2n";
const OTHER: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExxHsYI9UnIswq1HBUE9zECyyjh0ixyp2O+1SfSzZmy";

fn key(text: &str) -> HostKey {
    HostKey::parse(text).unwrap()
}

/// **One row of ADR 0123 §7's table**: a finding, the cause the window draws its card by, the
/// words the person reads, and the one way on.
struct Row {
    finding: Finding,
    route: &'static str,
    cause: &'static str,
    words: String,
    next: ActionId,
}

/// **Every finding**, each once. [`variant`] fails to build where one is added without a row.
fn every() -> Vec<Row> {
    let ssh = "ssh://me@build-box";
    let tls = "tcp://build-box:2376";
    let row = |finding, route, cause, words: &str, next| Row {
        finding,
        route,
        cause,
        words: words.to_owned(),
        next,
    };
    vec![
        row(
            Finding::NoSuchName {
                host: "build-box".into(),
            },
            ssh,
            "no-such-name",
            "purlis couldn't find a machine called build-box. Check the name, or pick it from \
             your SSH hosts.",
            ActionId::ChangeMachine,
        ),
        row(
            Finding::NoAnswer {
                host: "build-box".into(),
                port: 22,
            },
            ssh,
            "no-answer",
            "build-box didn't answer on port 22. Check that it's on and on this network, and \
             connect your VPN if you use one.",
            ActionId::CheckAgain,
        ),
        row(
            Finding::NotMet {
                host: "build-box".into(),
                said: "kex_exchange_identification: Connection closed".into(),
            },
            ssh,
            "not-met",
            "purlis couldn't reach SSH on build-box (it said: kex_exchange_identification: \
             Connection closed). Check that SSH is running there, then check again.",
            ActionId::CheckAgain,
        ),
        row(
            Finding::KeyChanged {
                host: "build-box".into(),
                pinned: key(KEY),
                now: key(OTHER),
            },
            ssh,
            "pool-pin-changed",
            &format!(
                "build-box shows a different key than the one you trusted ({} now, {} trusted). \
                 This happens when the machine is reinstalled, and can mean something else is \
                 answering in its place, so purlis connects to nothing there until you check it \
                 and trust it again.",
                key(OTHER).fingerprint(),
                key(KEY).fingerprint()
            ),
            ActionId::CheckAndTrustAgain,
        ),
        row(
            Finding::SignInRefused {
                host: "build-box".into(),
            },
            ssh,
            "sign-in-refused",
            "build-box didn't accept your sign-in. purlis signs in with your SSH agent and never \
             asks for a password. Add your key to the agent with `ssh-add`, then check again.",
            ActionId::CheckAgain,
        ),
        row(
            Finding::NoEngine {
                host: "build-box".into(),
                user: "me".into(),
            },
            ssh,
            "no-engine",
            "build-box has no Docker running that me can use. Start Docker there, or name its \
             socket in the address, then check again.",
            ActionId::CheckAgain,
        ),
        row(
            Finding::NotAllowed {
                host: "build-box".into(),
                user: "me".into(),
                socket: "/var/run/docker.sock".into(),
            },
            ssh,
            "not-allowed",
            "me on build-box isn't allowed to use Docker. On build-box, add me to the docker \
             group, or use rootless Docker or Podman, then check again.",
            ActionId::CheckAgain,
        ),
        row(
            Finding::EngineSilent {
                host: "build-box".into(),
                said: String::new(),
            },
            ssh,
            "engine-silent",
            "Docker on build-box didn't answer through SSH. Check that Docker is running there, \
             then check again.",
            ActionId::CheckAgain,
        ),
        row(
            Finding::ForwardingRefused {
                host: "build-box".into(),
            },
            ssh,
            "forwarding-refused",
            "The SSH server on build-box doesn't allow forwarding, and purlis reaches Docker \
             there by forwarding its socket. Ask its admin to allow it for your account \
             (`AllowTcpForwarding`, `AllowStreamLocalForwarding` and `DisableForwarding` in \
             sshd_config, or `restrict` and `no-port-forwarding` on your key), then check again.",
            ActionId::CheckAgain,
        ),
        row(
            Finding::EngineChanged {
                host: "build-box".into(),
                pinned: "AAAA:BBBB".into(),
                now: "CCCC:DDDD".into(),
                podman: false,
            },
            ssh,
            "pool-engine-changed",
            "The Docker engine on build-box isn't the one you trusted: it was reset or \
             reinstalled. purlis makes and links no runner there until you check it and trust \
             it again. Runners made on the old engine aren't on this one.",
            ActionId::CheckAndTrustAgain,
        ),
        row(
            Finding::NeedsPodman {
                host: "build-box".into(),
            },
            ssh,
            "needs-podman",
            &format!(
                "build-box runs Podman, and purlis reaches Podman with Podman's own client. \
                 Install Podman on {}, then check again.",
                super::super::this_machine()
            ),
            ActionId::CheckAgain,
        ),
        row(
            Finding::TlsFile {
                host: "build-box".into(),
                file: "/home/me/.docker/ca.pem".into(),
                why: "isn't there".into(),
            },
            tls,
            "tls-file",
            "/home/me/.docker/ca.pem isn't there. Check the file named under Options, then \
             check again.",
            ActionId::CheckAgain,
        ),
        row(
            Finding::CaChanged {
                host: "build-box".into(),
                pinned: "AA:BB".into(),
                now: "CC:DD".into(),
                file: "/home/me/.docker/ca.pem".into(),
            },
            tls,
            "pool-pin-changed",
            "/home/me/.docker/ca.pem no longer holds the CA you trusted for build-box (CC:DD \
             now, AA:BB trusted). This happens when the CA is replaced, and can mean something \
             else is answering in its place, so purlis connects to nothing there until you \
             check it and trust it again.",
            ActionId::CheckAndTrustAgain,
        ),
        row(
            Finding::NotSignedByCa {
                host: "build-box".into(),
                ca: "/home/me/.docker/ca.pem".into(),
            },
            tls,
            "tls-not-signed",
            "build-box's certificate isn't signed by the CA you chose. Check \
             /home/me/.docker/ca.pem: it must be the CA Docker's certificates on build-box are \
             signed with.",
            ActionId::CheckAgain,
        ),
        row(
            Finding::ServerExpired {
                host: "build-box".into(),
            },
            tls,
            "tls-expired",
            "build-box's certificate has expired. Make Docker a new one on build-box, the file \
             its --tlscert names there, then check again.",
            ActionId::CheckAgain,
        ),
        row(
            Finding::WrongName {
                host: "build-box".into(),
                name: "other-box".into(),
            },
            tls,
            "tls-wrong-name",
            "build-box's certificate is for another name (other-box). Connect by that name, or \
             check the file Docker's --tlscert names on build-box.",
            ActionId::ChangeMachine,
        ),
        row(
            Finding::ClientRefused {
                host: "build-box".into(),
                cert: "/home/me/.docker/cert.pem".into(),
            },
            tls,
            "tls-client-refused",
            "Docker on build-box didn't accept your client certificate. Check \
             /home/me/.docker/cert.pem: it must be signed by the CA Docker on build-box trusts, \
             the file its --tlscacert names.",
            ActionId::CheckAgain,
        ),
        row(
            Finding::ClientExpired {
                host: "build-box".into(),
                cert: "/home/me/.docker/cert.pem".into(),
            },
            tls,
            "tls-client-expired",
            "Your client certificate has expired. Check /home/me/.docker/cert.pem, make a new \
             one, then check again.",
            ActionId::CheckAgain,
        ),
        row(
            Finding::Unconfined {
                host: "build-box".into(),
                podman: false,
            },
            ssh,
            "unconfined",
            "Docker on build-box doesn't give containers its built-in seccomp profile, so purlis \
             makes no runner there. On build-box, set the daemon's seccomp profile back to its \
             default, then check again.",
            ActionId::CheckAgain,
        ),
        row(
            Finding::Architecture {
                host: "build-box".into(),
                platform: "linux/s390x".into(),
                built_for: RUNNER_PLATFORMS
                    .iter()
                    .map(|one| (*one).to_owned())
                    .collect(),
            },
            ssh,
            "unsupported-architecture",
            "build-box is linux/s390x, and the runner image is built for linux/amd64 and \
             linux/arm64. Pick a machine that is one of those.",
            ActionId::ChangeMachine,
        ),
        row(
            Finding::DiskFull {
                host: "build-box".into(),
                free: Some(512_000_000),
            },
            ssh,
            "disk-full",
            "Docker's disk on build-box is full: it has 512 MB free, and a runner needs at least \
             1.0 GB, so purlis makes no runner there. To make room, run docker system df on \
             build-box to see what takes it, then docker system prune there to remove what \
             nothing uses, or give Docker there a bigger disk. Then check again.",
            ActionId::CheckAgain,
        ),
        row(
            Finding::ImageUnreachable {
                host: "build-box".into(),
                registry: "ghcr.io".into(),
            },
            ssh,
            "image-unreachable",
            "build-box couldn't download the runner image. Check that build-box can reach \
             ghcr.io, then check again.",
            ActionId::CheckAgain,
        ),
        row(
            Finding::ContextMoved {
                name: "builder".into(),
            },
            ssh,
            "context-moved",
            "Your context builder now points somewhere else than this pool. Connect again to \
             follow it, or keep the pool as it is.",
            ActionId::ConnectAgain,
        ),
        row(
            Finding::Here("purlis found no ssh on this machine.".into()),
            ssh,
            "here",
            "purlis found no ssh on this machine.",
            ActionId::CheckAgain,
        ),
    ]
}

/// **Each variant's place**, with no arm to spare: a finding added without a row in [`every`]
/// fails to build here.
fn variant(finding: &Finding) -> usize {
    match finding {
        Finding::NoSuchName { .. } => 0,
        Finding::NoAnswer { .. } => 1,
        Finding::NotMet { .. } => 2,
        Finding::KeyChanged { .. } => 3,
        Finding::SignInRefused { .. } => 4,
        Finding::NoEngine { .. } => 5,
        Finding::NotAllowed { .. } => 6,
        Finding::EngineSilent { .. } => 7,
        Finding::ForwardingRefused { .. } => 8,
        Finding::EngineChanged { .. } => 9,
        Finding::NeedsPodman { .. } => 10,
        Finding::TlsFile { .. } => 11,
        Finding::CaChanged { .. } => 12,
        Finding::NotSignedByCa { .. } => 13,
        Finding::ServerExpired { .. } => 14,
        Finding::WrongName { .. } => 15,
        Finding::ClientRefused { .. } => 16,
        Finding::ClientExpired { .. } => 17,
        Finding::Unconfined { .. } => 18,
        Finding::Architecture { .. } => 19,
        Finding::DiskFull { .. } => 20,
        Finding::ImageUnreachable { .. } => 21,
        Finding::ContextMoved { .. } => 22,
        Finding::Here(_) => 23,
    }
}

const VARIANTS: usize = 24;

fn facts(route: &str, finding: &Finding) -> Facts {
    let tls = crate::tlsroute::is_tcp(route).then(|| crate::tlsroute::Files {
        ca: "/home/me/.docker/ca.pem".into(),
        cert: "/home/me/.docker/cert.pem".into(),
        key: crate::tlsroute::KeyFrom::File("/home/me/.docker/key.pem".into()),
    });
    Facts {
        engine: Engine::Ready,
        elsewhere: None,
        connect: Some(Connecting {
            route: Some(route.to_owned()),
            tls,
            finding: Some(finding.clone()),
            ..Connecting::default()
        }),
        pool: None,
        identity: Identity::None,
        repos: true,
        cluster: super::super::cluster::ClusterFacts::default(),
    }
}

fn connect_of(row: &Row) -> Step {
    let choices = Choices {
        kind: Some(Kind::RemoteDocker),
        ..Choices::default()
    };
    of(
        Kind::RemoteDocker,
        &facts(row.route, &row.finding),
        &choices,
    )
    .step(StepId::Connect)
    .unwrap()
    .clone()
}

#[test]
fn the_table_holds_every_finding_once() {
    let mut seen = [false; VARIANTS];
    for row in every() {
        let at = variant(&row.finding);
        assert!(!seen[at], "{:?} is in the table twice", row.finding);
        seen[at] = true;
    }
    assert!(seen.iter().all(|one| *one), "{seen:?}");
}

/// **ADR 0123 §7, row by row** (#6272): Pool setup's Connect step stops on each finding with
/// its card's cause, says what happened and one thing to do in the record's words, offers that
/// one way on, and shows its commands under See what this does.
#[test]
fn pool_setup_says_each_finding_with_one_next_step_and_its_commands() {
    for row in every() {
        let step = connect_of(&row);
        assert_eq!(step.state, StepState::Attention, "{:?}", row.finding);
        assert_eq!(step.cause, Some(row.cause), "{:?}", row.finding);
        assert_eq!(step.findings, vec![row.words.clone()], "{:?}", row.finding);
        let next = step
            .next
            .as_ref()
            .unwrap_or_else(|| panic!("{:?}", row.finding));
        assert_eq!(next.id, row.next, "{:?}", row.finding);
        assert!(!next.label.is_empty() && !next.does.is_empty());
        let see = step
            .see
            .as_ref()
            .unwrap_or_else(|| panic!("{:?} has no See what this does", row.finding));
        assert!(!see.text.trim().is_empty(), "{:?}", row.finding);
        assert!(!see.note.trim().is_empty(), "{:?}", row.finding);
    }
}

/// **The window and `purlis pool doctor` say the same** (#6272): doctor's lines are the step's
/// own sentence, its way on and its commands, word for word.
#[test]
fn doctor_says_each_finding_in_the_windows_words_with_its_next_step_and_commands() {
    for row in every() {
        let step = connect_of(&row);
        let machine = Connecting {
            finding: None,
            ..facts(row.route, &row.finding).connect.unwrap()
        };
        let (said, code) = said_for("ci", &machine, &Err(row.finding.clone()));
        assert_eq!(code, 1, "{:?}", row.finding);
        let next = step.next.as_ref().unwrap();
        let see = step.see.as_ref().unwrap();
        // A changed pin is said in its Notice's words, which the window shows above the step.
        let words = super::super::pinchange::of("ci", &row.finding)
            .map_or_else(|| row.words.clone(), |change| change.said);
        for line in [
            words.as_str(),
            &format!("Next: {}. {}", next.label, next.does),
            "See what this does:",
            see.note.as_str(),
        ] {
            assert!(
                said.contains(line),
                "{:?}: doctor said\n{said}\nwithout {line:?}",
                row.finding
            );
        }
        for command in see.text.lines() {
            assert!(said.contains(&format!("  {command}\n")), "{said}");
        }
    }
}

#[test]
fn doctor_says_a_pool_that_holds_with_the_links_round_trip_and_exits_0() {
    let (said, code) = said_for(
        "ci",
        &Connecting::default(),
        &Ok(Healthy {
            host: "build-box".into(),
            round_trip: std::time::Duration::from_micros(48_400),
        }),
    );
    assert_eq!(code, 0);
    assert_eq!(
        said,
        "Pool ci: build-box is the machine you trusted, and its engine the one you trusted. \
         Docker there confines runners, has the runner image and room for a runner.\n\
         A round trip to build-box takes 48 ms.\n"
    );
}

#[test]
fn a_platform_is_said_as_docker_names_one_whatever_the_engine_calls_its_architecture() {
    assert_eq!(platform_of("linux", "x86_64"), "linux/amd64");
    assert_eq!(platform_of("linux", "aarch64"), "linux/arm64");
    assert_eq!(platform_of("linux", "arm64"), "linux/arm64");
    assert_eq!(platform_of("Linux", "s390x"), "linux/s390x");
    assert!(runs_the_runner_image("linux/amd64"));
    assert!(!runs_the_runner_image("linux/riscv64"));
}

#[test]
fn an_images_registry_is_its_first_part_where_that_names_a_host_else_docker_hub() {
    assert_eq!(
        registry_of("ghcr.io/purlis/purlis-runner@sha256:00"),
        "ghcr.io"
    );
    assert_eq!(
        registry_of("localhost:5000/purlis-runner@sha256:00"),
        "localhost:5000"
    );
    assert_eq!(registry_of("purlis/runner@sha256:00"), "docker.io");
    assert_eq!(registry_of("runner@sha256:00"), "docker.io");
}

#[test]
fn a_round_trip_is_said_in_whole_milliseconds_never_less_than_one() {
    use std::time::Duration;
    assert_eq!(
        round_trip_said("build-box", Duration::from_micros(48_900)),
        "A round trip to build-box takes 48 ms."
    );
    assert_eq!(
        round_trip_said("build-box", Duration::from_micros(300)),
        "A round trip to build-box takes 1 ms."
    );
}

#[test]
fn the_middle_of_three_round_trips_is_the_one_kept() {
    use std::time::Duration;
    let ms = Duration::from_millis;
    assert_eq!(middle(&mut [ms(90), ms(40), ms(50)]), ms(50));
}
