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
                own_image: false,
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
             build-box to see what takes it, then docker image prune there to remove images \
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
            passed: Passed::every(),
        }),
    );
    assert_eq!(code, 0);
    assert_eq!(
        said,
        "Pool ci: build-box is the machine you trusted, and its engine the one you trusted. \
         Docker there confines runners and has the runner image and room for a runner.\n\
         A docker command on build-box answers in 48 ms, from start to answer.\n"
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
        "A docker command on build-box answers in 48 ms, from start to answer."
    );
    assert_eq!(
        round_trip_said("build-box", Duration::from_micros(300)),
        "A docker command on build-box answers in 1 ms, from start to answer."
    );
}

#[test]
fn the_middle_of_three_round_trips_is_the_one_kept() {
    use std::time::Duration;
    let ms = Duration::from_millis;
    assert_eq!(middle(&mut [ms(90), ms(40), ms(50)]), ms(50));
}

fn trusted_over_ssh(finding: Finding) -> Facts {
    let mut facts = facts("ssh://me@build-box", &finding);
    if let Some(connect) = facts.connect.as_mut() {
        connect.identity = Some("/home/me/.ssh/id_builds".into());
        connect.trusted = Some(crate::sshroute::Pin {
            host_key: key(KEY),
            engine: crate::sshroute::EnginePin::Docker("ABCD:EFGH:IJKL".into()),
        });
    }
    facts
}

fn trusted_over_tls(finding: Finding) -> Facts {
    let mut facts = facts("tcp://build-box:2376", &finding);
    if let Some(connect) = facts.connect.as_mut() {
        connect.tls = Some(crate::tlsroute::Files {
            ca: "/home/me/.docker/ca.pem".into(),
            cert: "/home/me/.docker/cert.pem".into(),
            key: crate::tlsroute::KeyFrom::Vault {
                plane: "/home/me/projects/builds".into(),
                vault: "builds-vault".into(),
                key: "docker-client-key".into(),
            },
        });
        connect.trusted_tls = Some(crate::tlsroute::Pin {
            ca: crate::tlsroute::Fingerprint::parse(&["AB"; 32].join(":")).unwrap(),
            engine: "ABCD:EFGH:IJKL".into(),
        });
    }
    facts
}

fn step_of(facts: &Facts) -> Step {
    let choices = Choices {
        kind: Some(Kind::RemoteDocker),
        ..Choices::default()
    };
    of(Kind::RemoteDocker, facts, &choices)
        .step(StepId::Connect)
        .unwrap()
        .clone()
}

/// **Send to your admin carries the route, the host and the fingerprints** (#6272, ADR 0123 §4),
/// with what the check found and the commands, and never a key or a value.
#[test]
fn send_to_your_admin_names_the_route_host_and_fingerprints_and_no_key_or_value() {
    let step = step_of(&trusted_over_ssh(Finding::NotAllowed {
        host: "build-box".into(),
        user: "me".into(),
        socket: "/var/run/docker.sock".into(),
    }));
    assert_eq!(step.also.as_ref().unwrap().id, ActionId::SendToAdmin);
    let words = step.admin.as_deref().unwrap();
    for part in [
        "ssh://me@build-box",
        "build-box",
        &key(KEY).fingerprint(),
        "ABCD:EFGH:IJKL",
        "me on build-box isn't allowed to use Docker",
        "sudo usermod -aG docker me",
        "It holds no password, token, key or credential.",
    ] {
        assert!(words.contains(part), "{words}\nwithout {part:?}");
    }
    assert!(!words.contains("id_builds"), "{words}");

    let step = step_of(&trusted_over_tls(Finding::NotSignedByCa {
        host: "build-box".into(),
        ca: "/home/me/.docker/ca.pem".into(),
    }));
    let words = step.admin.as_deref().unwrap();
    assert!(words.contains("tcp://build-box:2376"), "{words}");
    assert!(words.contains(&["AB"; 32].join(":")), "{words}");
    for secret in ["builds-vault", "docker-client-key", "key.pem"] {
        assert!(!words.contains(secret), "{words}\nholds {secret:?}");
    }
}

/// What the person mends themselves (their own agent, the address they typed, the files they
/// named) has no admin to send to.
#[test]
fn a_finding_the_person_mends_themselves_offers_no_admin() {
    for finding in [
        Finding::SignInRefused {
            host: "build-box".into(),
        },
        Finding::NoSuchName {
            host: "build-box".into(),
        },
        Finding::Here("purlis found no ssh on this machine.".into()),
    ] {
        let step = step_of(&trusted_over_ssh(finding.clone()));
        assert_eq!(step.admin, None, "{finding:?}");
        assert_eq!(step.also, None, "{finding:?}");
    }
}

/// **Get help's chat is told the route, the host and the fingerprints** (#6272), and never a key
/// or a value.
#[test]
fn get_help_names_the_route_host_and_fingerprints_and_no_key_or_value() {
    let choices = Choices {
        kind: Some(Kind::RemoteDocker),
        ..Choices::default()
    };
    let setup = of(
        Kind::RemoteDocker,
        &trusted_over_tls(Finding::ServerExpired {
            host: "build-box".into(),
        }),
        &choices,
    );
    let opening = super::super::help::opening(&setup, StepId::Connect, None).unwrap();
    for part in [
        "over Docker's API over TLS",
        "Route: tcp://build-box:2376",
        "Machine: build-box",
        &format!("Its CA, trusted: {}", ["AB"; 32].join(":")),
        "Its engine, trusted: ABCD:EFGH:IJKL",
    ] {
        assert!(opening.contains(part), "{opening}\nwithout {part:?}");
    }
    for secret in ["builds-vault", "docker-client-key"] {
        assert!(!opening.contains(secret), "{opening}\nholds {secret:?}");
    }
    let setup = of(
        Kind::RemoteDocker,
        &trusted_over_ssh(Finding::NoEngine {
            host: "build-box".into(),
            user: "me".into(),
        }),
        &choices,
    );
    let opening = super::super::help::opening(&setup, StepId::Connect, None).unwrap();
    assert!(opening.contains("over SSH"), "{opening}");
    assert!(
        opening.contains(&format!("Its key, trusted: {}", key(KEY).fingerprint())),
        "{opening}"
    );
    assert!(!opening.contains("id_builds"), "{opening}");
}

/// **Try it's line for the link** (ADR 0123 §6): its round trip, measured, or why it couldn't be.
#[test]
fn try_it_says_the_links_round_trip_or_why_it_couldnt_be_measured() {
    let measured = link_check("build-box", &Ok(std::time::Duration::from_micros(48_400)));
    assert_eq!(measured.about, CheckOf::Link);
    assert_eq!(measured.verdict, Verdict::Ok);
    assert_eq!(
        measured.title,
        "A docker command on build-box answers in 48 ms, from start to answer."
    );
    assert_eq!(measured.more, None);
    let finding = Finding::NoAnswer {
        host: "build-box".into(),
        port: 22,
    };
    let unmeasured = link_check("build-box", &Err(finding.clone()));
    assert_eq!(unmeasured.verdict, Verdict::Missing);
    assert_eq!(
        unmeasured.title,
        "purlis couldn't time a docker command on build-box"
    );
    assert_eq!(
        unmeasured.more.as_deref(),
        Some(super::super::connecting::said(&finding).as_str())
    );
    assert_eq!(serde_json::to_value(CheckOf::Link).unwrap(), "link");
}

/// **An engine on another machine, stood in for by a `docker` script** (#6272): `security`
/// answers `info --format '{{json .SecurityOptions}}'` (none, and the engine is not there),
/// `info` answers `info --format '{{json .}}'`, `inspect` answers `image inspect` (none, and the
/// image is not there), a pull says `pull-said` and exits with `pull-exit`, keeping `pulled` as
/// the image where it succeeds, and the image's `df` prints `df`, says `df-said` and exits with
/// `df-exit`. Every run is kept in `log`.
struct Stand {
    dir: tempfile::TempDir,
}

const DIGEST: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

impl Stand {
    fn new() -> Stand {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().display().to_string();
        let script = format!(
            "#!/bin/sh\nd='{d}'\necho \"$*\" >> \"$d/log\"\ncase \"$*\" in\n\
             *'--entrypoint df '*) cat \"$d/df\" 2>/dev/null; cat \"$d/df-said\" >&2 2>/dev/null; \
             exit \"$(cat \"$d/df-exit\" 2>/dev/null || echo 0)\" ;;\n\
             *SecurityOptions*) [ -f \"$d/security\" ] || {{ echo 'Cannot connect to the Docker \
             daemon' >&2; exit 1; }}; cat \"$d/security\" ;;\n\
             *Host.Security*) echo 'template: no field Host' >&2; exit 1 ;;\n\
             *'{{{{json .}}}}'*) cat \"$d/info\" ;;\n\
             *'image inspect'*) [ -f \"$d/inspect\" ] || {{ echo '[]'; echo 'Error: No such \
             image' >&2; exit 1; }}; cat \"$d/inspect\" ;;\n\
             *'pull '*) cat \"$d/pull-said\" >&2 2>/dev/null; code=$(cat \"$d/pull-exit\" \
             2>/dev/null || echo 0); [ \"$code\" = 0 ] && cp \"$d/pulled\" \"$d/inspect\"; \
             exit $code ;;\n\
             esac\n"
        );
        stand_in::program(dir.path(), "docker", &script);
        let stand = Stand { dir };
        stand.set(
            "security",
            r#"["name=seccomp,profile=builtin","name=cgroupns"]"#,
        );
        stand.set("info", r#"{"OSType":"linux","Architecture":"x86_64"}"#);
        stand.set("inspect", r#"[{"Os":"linux","Architecture":"amd64"}]"#);
        stand.set("pulled", r#"[{"Os":"linux","Architecture":"amd64"}]"#);
        stand.set(
            "df",
            "Filesystem 1024-blocks Used Available Capacity Mounted on\n\
             overlay 104857600 1048576 103809024 2% /\n",
        );
        stand
    }

    fn docker(&self) -> Docker {
        Docker::at(self.dir.path().join("docker")).with_settings_in(self.dir.path().join("s"))
    }

    fn set(&self, name: &str, text: &str) {
        std::fs::write(self.dir.path().join(name), text).unwrap();
    }

    fn gone(&self, name: &str) {
        let _ = std::fs::remove_file(self.dir.path().join(name));
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("log")).unwrap_or_default()
    }
}

fn ours() -> Image {
    Image::pinned(&format!("ghcr.io/purlis/purlis-runner@{DIGEST}")).unwrap()
}

fn theirs() -> Image {
    Image::pinned(&format!("registry.example.com/team/runner@{DIGEST}")).unwrap()
}

/// **An engine that answers with another seccomp profile doesn't confine runners**; one that
/// doesn't answer at all is said to be silent, never unconfined (#6272 review).
#[test]
fn an_engine_that_doesnt_answer_is_silent_and_one_with_another_profile_is_unconfined() {
    let stand = Stand::new();
    stand.set("security", r#"["name=seccomp,profile=unconfined"]"#);
    assert!(matches!(
        engine(&stand.docker(), "build-box", &ours()),
        Err(Finding::Unconfined { podman: false, .. })
    ));
    stand.gone("security");
    match engine(&stand.docker(), "build-box", &ours()) {
        Err(Finding::EngineSilent { host, said }) => {
            assert_eq!(host, "build-box");
            assert!(
                said.contains("Cannot connect to the Docker daemon"),
                "{said}"
            );
        }
        other => panic!("{other:?}"),
    }
}

/// **A download that fills the disk is a full disk, not a registry out of reach** (#6272
/// review): Docker's own words name it, as #6205 reads them.
#[test]
fn a_download_that_fills_the_disk_is_a_full_disk() {
    let stand = Stand::new();
    stand.gone("inspect");
    stand.set("pull-exit", "1");
    stand.set(
        "pull-said",
        "failed to register layer: write /var/lib/docker/overlay2/x: no space left on device",
    );
    assert_eq!(
        engine(&stand.docker(), "build-box", &ours()),
        Err(Finding::DiskFull {
            host: "build-box".into(),
            free: None
        })
    );
    stand.set(
        "pull-said",
        "Get \"https://ghcr.io/v2/\": dial tcp: i/o timeout",
    );
    assert_eq!(
        engine(&stand.docker(), "build-box", &ours()),
        Err(Finding::ImageUnreachable {
            host: "build-box".into(),
            registry: "ghcr.io".into()
        })
    );
}

/// **A disk that couldn't be measured passes, and is said so** (#6272 review): nothing is held
/// up, and nothing says there is room.
#[test]
fn a_disk_that_couldnt_be_measured_passes_and_is_said_so() {
    let stand = Stand::new();
    assert_eq!(
        engine(&stand.docker(), "build-box", &ours()),
        Ok(Passed::every())
    );
    stand.set("df-exit", "127");
    stand.set(
        "df-said",
        "exec: \"df\": executable file not found in $PATH",
    );
    let passed = engine(&stand.docker(), "build-box", &ours()).unwrap();
    assert!(passed.disk_unmeasured, "{passed:?}");
    assert!(!passed.checked.contains(&"disk-full"), "{passed:?}");
    let (said, code) = said_for(
        "ci",
        &Connecting::default(),
        &Ok(Healthy {
            host: "build-box".into(),
            round_trip: std::time::Duration::from_millis(48),
            passed,
        }),
    );
    assert_eq!(code, 0);
    assert!(!said.contains("room for a runner"), "{said}");
    assert!(
        said.contains("purlis couldn't measure Docker's disk on build-box"),
        "{said}"
    );
}

/// **The person's own image, built for another platform, is theirs to change** (#6272 review):
/// the way on is another image, on a new pool on the same machine, never another machine.
#[test]
fn an_image_of_your_own_built_for_another_platform_offers_another_image() {
    let stand = Stand::new();
    stand.set("inspect", r#"[{"Os":"linux","Architecture":"arm64"}]"#);
    let finding = engine(&stand.docker(), "build-box", &theirs()).unwrap_err();
    assert_eq!(
        finding,
        Finding::Architecture {
            host: "build-box".into(),
            platform: "linux/amd64".into(),
            built_for: vec!["linux/arm64".into()],
            own_image: true,
        }
    );
    let (words, next) = super::super::connecting::said_and_next(&finding);
    assert_eq!(
        words,
        "build-box is linux/amd64, and your image is built for linux/arm64. Use an image built \
         for linux/amd64, on a new pool on build-box."
    );
    assert_eq!(next.unwrap().id, ActionId::ChangeImage);
    let step = step_of(&trusted_over_ssh(finding));
    assert_eq!(step.next.as_ref().unwrap().id, ActionId::ChangeImage);
    assert_eq!(
        step.prefill,
        Some(Prefill {
            route: "ssh://me@build-box".into(),
            context: None
        })
    );
    // purlis's own image on a machine it isn't built for: another machine.
    stand.set("info", r#"{"OSType":"linux","Architecture":"riscv64"}"#);
    stand.gone("inspect");
    assert!(matches!(
        engine(&stand.docker(), "build-box", &ours()),
        Err(Finding::Architecture {
            own_image: false,
            ..
        })
    ));
}

/// **Connect's checks of the engine, before any pool is kept** (#6272 review): that it confines
/// runners and that purlis's runner image is built for its platform, read with `docker info`
/// alone: no image is looked for or downloaded.
#[test]
fn connect_checks_the_engine_with_docker_info_alone() {
    let stand = Stand::new();
    assert_eq!(
        on_connect(&stand.docker(), "build-box"),
        Ok(Passed {
            checked: vec!["unconfined", "unsupported-architecture"],
            disk_unmeasured: false,
        })
    );
    stand.set("info", r#"{"OSType":"linux","Architecture":"riscv64"}"#);
    assert_eq!(
        on_connect(&stand.docker(), "build-box"),
        Err(Finding::Architecture {
            host: "build-box".into(),
            platform: "linux/riscv64".into(),
            built_for: vec!["linux/amd64".into(), "linux/arm64".into()],
            own_image: false,
        })
    );
    stand.set("security", r#"["name=seccomp,profile=unconfined"]"#);
    assert!(matches!(
        on_connect(&stand.docker(), "build-box"),
        Err(Finding::Unconfined { .. })
    ));
    let log = stand.log();
    assert!(
        !log.contains("image") && !log.contains("pull") && !log.contains("run"),
        "{log}"
    );
}

/// **What doctor found is kept for the machine's route** (#6272 review), so the next read shows
/// it, until doctor runs there again.
#[test]
fn what_doctor_found_is_laid_over_the_next_read_of_its_machine() {
    let route = "ssh://me@kept-for-the-next-read";
    let mut connect = Connecting {
        route: Some(route.into()),
        ..Connecting::default()
    };
    lay_over(&mut connect);
    assert_eq!(connect.finding, None);
    assert!(connect.checked.is_empty());
    let unconfined = Finding::Unconfined {
        host: "kept-for-the-next-read".into(),
        podman: false,
    };
    note(route, &Err(unconfined.clone()));
    lay_over(&mut connect);
    assert_eq!(connect.finding, Some(unconfined));
    note(route, &Ok(Passed::every()));
    let mut connect = Connecting {
        route: Some(route.into()),
        ..Connecting::default()
    };
    lay_over(&mut connect);
    assert_eq!(connect.finding, None);
    assert_eq!(connect.checked, Passed::every().checked);
    // A finding of reaching the machine, read now, stands over what doctor found before.
    note(
        route,
        &Err(Finding::DiskFull {
            host: "kept-for-the-next-read".into(),
            free: None,
        }),
    );
    let silent = Finding::EngineSilent {
        host: "kept-for-the-next-read".into(),
        said: String::new(),
    };
    let mut connect = Connecting {
        route: Some(route.into()),
        finding: Some(silent.clone()),
        ..Connecting::default()
    };
    lay_over(&mut connect);
    assert_eq!(connect.finding, Some(silent));
}

/// **The machine's checked lines reach the window** (#6272 review): which of doctor's engine
/// checks passed, and whether the disk could be measured, so no line reads as passed unrun.
#[test]
fn the_window_is_told_which_engine_checks_passed() {
    let mut facts = trusted_over_ssh(Finding::Here(String::new()));
    if let Some(connect) = facts.connect.as_mut() {
        connect.finding = None;
        connect.checked = vec!["unconfined", "unsupported-architecture"];
    }
    let step = step_of(&facts);
    assert_eq!(step.state, StepState::Done);
    let machine = step.machine.unwrap();
    assert_eq!(
        machine.checked,
        vec!["unconfined", "unsupported-architecture"]
    );
    assert!(!machine.disk_unmeasured);
}

/// **The round trip says what it covers** (#6272 review): one `docker` command asked over the
/// link, from start to answer, purlis's own start of it included.
#[test]
fn the_round_trip_says_it_is_one_docker_command_from_start_to_answer() {
    assert_eq!(
        round_trip_said("build-box", std::time::Duration::from_micros(48_900)),
        "A docker command on build-box answers in 48 ms, from start to answer."
    );
}

/// **The context is checked before anything is downloaded** (#6272 review): a pool whose context
/// moved is told so without its image being looked for.
#[test]
fn a_moved_context_is_found_before_the_image_is_looked_for() {
    let stand = Stand::new();
    let pool = crate::runners::Pool::new("moved", ours(), crate::runners::Set::default())
        .unwrap()
        .on_engine(Some(crate::runners::RemoteEngine::Tls(
            crate::tlsroute::Remote {
                route: crate::tlsroute::Route::parse("tcp://build-box:2376").unwrap(),
                files: crate::tlsroute::Files {
                    ca: "/nowhere/ca.pem".into(),
                    cert: "/nowhere/cert.pem".into(),
                    key: crate::tlsroute::KeyFrom::File("/nowhere/key.pem".into()),
                },
                pin: crate::tlsroute::Pin {
                    ca: crate::tlsroute::Fingerprint::parse(&["AB"; 32].join(":")).unwrap(),
                    engine: "ABCD".into(),
                },
            },
        )))
        .from_docker_context(Some("builder".into()));
    let found = after_reaching(&stand.docker(), &pool, "build-box", &|| {
        vec![crate::dockercontext::DockerContext {
            name: "builder".into(),
            current: false,
            endpoint: crate::dockercontext::Endpoint::Ssh(
                crate::sshroute::Route::parse("ssh://me@elsewhere").unwrap(),
            ),
        }]
    });
    assert_eq!(
        found,
        Err(Finding::ContextMoved {
            name: "builder".into()
        })
    );
    assert!(!stand.log().contains("image"), "{}", stand.log());
}

/// A pool over TLS whose CA, certificate and key are files of the person's, under their home.
fn trusted_over_tls_files(finding: Finding) -> Facts {
    let mut facts = trusted_over_tls(finding);
    if let Some(connect) = facts.connect.as_mut() {
        connect.tls = Some(crate::tlsroute::Files {
            ca: "/home/me/.docker/build-box/ca.pem".into(),
            cert: "/home/me/.docker/build-box/cert.pem".into(),
            key: crate::tlsroute::KeyFrom::File("/home/me/.docker/build-box/key.pem".into()),
        });
    }
    facts
}

/// **Neither the admin's words nor Get help's carry a path of this machine's** (#6272 review):
/// not the client key's, the certificate's, the CA's or the SSH key's. Each stands as a
/// placeholder the reader fills in.
#[test]
fn no_local_path_reaches_the_admin_or_get_help() {
    let choices = Choices {
        kind: Some(Kind::RemoteDocker),
        ..Choices::default()
    };
    for finding in [
        Finding::EngineSilent {
            host: "build-box".into(),
            said: "error during connect: open /home/me/.docker/build-box/key.pem".into(),
        },
        Finding::NotSignedByCa {
            host: "build-box".into(),
            ca: "/home/me/.docker/build-box/ca.pem".into(),
        },
        Finding::ClientRefused {
            host: "build-box".into(),
            cert: "/home/me/.docker/build-box/cert.pem".into(),
        },
        Finding::Unconfined {
            host: "build-box".into(),
            podman: false,
        },
    ] {
        let facts = trusted_over_tls_files(finding.clone());
        let step = step_of(&facts);
        let words = step
            .admin
            .as_deref()
            .unwrap_or_else(|| panic!("{finding:?} has no words for the admin"));
        let setup = of(Kind::RemoteDocker, &facts, &choices);
        let help = super::super::help::opening(&setup, StepId::Connect, None).unwrap();
        for (what, text) in [("admin", words), ("Get help", help.as_str())] {
            assert!(
                !text.contains("/home/me"),
                "{finding:?}: {what}'s text\n{text}"
            );
        }
        assert!(
            words.contains("<your CA file>") || words.contains("<your client certificate>"),
            "{finding:?}: {words}"
        );
    }
    let facts = trusted_over_tls_files(Finding::Unconfined {
        host: "build-box".into(),
        podman: false,
    });
    let words = step_of(&facts).admin.unwrap();
    assert!(words.contains("--tlskey '<your client key>'"), "{words}");
    // The window's own See what this does keeps the person's paths: they run it here.
    let see = step_of(&facts).see.unwrap();
    assert!(
        see.text.contains("/home/me/.docker/build-box/key.pem"),
        "{}",
        see.text
    );
}

/// **Get help tells the commands that only read from those that change something** (#6272
/// review): adding to the docker group, starting Docker and pruning are never "for reading".
#[test]
fn get_help_keeps_what_changes_something_apart_from_what_only_reads() {
    let choices = Choices {
        kind: Some(Kind::RemoteDocker),
        ..Choices::default()
    };
    for (finding, changes) in [
        (
            Finding::NotAllowed {
                host: "build-box".into(),
                user: "me".into(),
                socket: "/var/run/docker.sock".into(),
            },
            "sudo usermod -aG docker me",
        ),
        (
            Finding::NoEngine {
                host: "build-box".into(),
                user: "me".into(),
            },
            "sudo systemctl enable --now docker",
        ),
        (
            Finding::DiskFull {
                host: "build-box".into(),
                free: None,
            },
            "image prune",
        ),
    ] {
        let setup = of(
            Kind::RemoteDocker,
            &trusted_over_ssh(finding.clone()),
            &choices,
        );
        let help = super::super::help::opening(&setup, StepId::Connect, None).unwrap();
        let (_, rest) = help
            .split_once("The commands that only read:")
            .unwrap_or_else(|| panic!("{finding:?}:\n{help}"));
        let (reads, changing) = rest
            .split_once("The commands that change something there, the person's to run:")
            .unwrap_or_else(|| panic!("{finding:?}:\n{help}"));
        assert!(!reads.contains(changes), "{finding:?}:\n{help}");
        assert!(changing.contains(changes), "{finding:?}:\n{help}");
        assert!(!help.contains("for reading only"), "{help}");
    }
}

/// **Every command purlis shows for a finding is sorted**: what only reads, and what changes
/// something, each line once.
#[test]
fn every_findings_commands_are_sorted_into_reading_and_changing() {
    for row in every() {
        let step = connect_of(&row);
        let see = step.see.unwrap();
        let (reads, changes) = super::super::connecting::sorted(&see);
        assert_eq!(
            reads.len() + changes.len(),
            see.text.lines().count(),
            "{:?}",
            row.finding
        );
        for line in &reads {
            for word in ["prune", "systemctl", "usermod", "pull "] {
                assert!(!line.contains(word), "{:?}: {line}", row.finding);
            }
        }
    }
}

/// **Check again says what it does** (#6272 review): it runs every check, and may download the
/// runner image there, which takes a while. It never claims to change nothing.
#[test]
fn check_again_says_it_may_download_the_runner_image() {
    for row in every() {
        let step = connect_of(&row);
        let next = step.next.unwrap();
        if next.id == ActionId::CheckAgain {
            assert!(!next.does.contains("changes nothing"), "{}", next.does);
            assert!(
                next.does.contains("downloads the runner image"),
                "{}",
                next.does
            );
        }
    }
}

/// **The full disk's way to make room touches only what no one needs** (#6272 review): it looks
/// first, then removes unused images, and never every stopped container on a shared machine.
#[test]
fn a_full_disk_is_mended_by_looking_first_and_removing_unused_images_alone() {
    let finding = Finding::DiskFull {
        host: "build-box".into(),
        free: None,
    };
    let (words, _) = super::super::connecting::said_and_next(&finding);
    let see = step_of(&trusted_over_ssh(finding)).see.unwrap();
    for text in [words.as_str(), see.text.as_str(), see.note.as_str()] {
        assert!(!text.contains("system prune"), "{text}");
    }
    assert!(see.text.contains("system df"), "{}", see.text);
    assert!(see.text.contains("image prune"), "{}", see.text);
    assert!(see.note.contains("stopped containers stay"), "{}", see.note);
}

/// **Which findings go to the machine's admin is named, never assumed** (#6272 review): a cause
/// added later goes nowhere until it is put in the list.
#[test]
fn only_the_findings_named_for_the_admin_go_to_the_admin() {
    use super::super::connecting::for_the_admin;
    for cause in [
        "no-engine",
        "not-allowed",
        "forwarding-refused",
        "unconfined",
        "disk-full",
        "image-unreachable",
        "tls-not-signed",
        "tls-client-refused",
    ] {
        assert!(for_the_admin(cause), "{cause}");
    }
    for cause in [
        "here",
        "not-a-route",
        "no-such-name",
        "sign-in-refused",
        "tls-file",
        "context-moved",
        "a-cause-added-later",
    ] {
        assert!(!for_the_admin(cause), "{cause}");
    }
}
