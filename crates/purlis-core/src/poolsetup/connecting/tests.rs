// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::super::*;
use super::*;

const KEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINiGzfjDDJsgYLYCmdkf8Fel9sE6Ma14Y2+c33Y/qZ2n";
const FINGERPRINT: &str = "SHA256:dmeCC9oOb+c7VYVBSUAdgQchZHZQyXKWgKnGQxfh2rA";
const OTHER: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExxHsYI9UnIswq1HBUE9zECyyjh0ixyp2O+1SfSzZmy";

fn key(text: &str) -> HostKey {
    HostKey::parse(text).unwrap()
}

fn remote_facts(connect: Connecting) -> Facts {
    Facts {
        engine: Engine::Ready,
        elsewhere: None,
        connect: Some(connect),
        pool: None,
        identity: Identity::None,
        repos: true,
        cluster: super::super::cluster::ClusterFacts::default(),
    }
}

fn chosen() -> Choices {
    Choices {
        kind: Some(Kind::RemoteDocker),
        ..Choices::default()
    }
}

fn routed(route: &str) -> Connecting {
    Connecting {
        route: Some(route.to_owned()),
        ..Connecting::default()
    }
}

fn connect_of(facts: &Facts) -> Step {
    of(Kind::RemoteDocker, facts, &chosen())
        .step(StepId::Connect)
        .unwrap()
        .clone()
}

fn found(finding: Finding) -> Step {
    connect_of(&remote_facts(Connecting {
        finding: Some(finding),
        ..routed("ssh://me@build-box")
    }))
}

#[test]
fn a_pool_on_another_machine_has_connect_between_where_and_pool() {
    assert_eq!(
        Kind::RemoteDocker.steps(),
        [
            StepId::Where,
            StepId::Connect,
            StepId::Pool,
            StepId::Signins,
            StepId::Identity,
            StepId::Try,
        ]
    );
    assert_eq!(
        serde_json::to_value(Kind::RemoteDocker).unwrap(),
        "remote-docker"
    );
    assert!(!StepId::Connect.skippable());
}

#[test]
fn connect_asks_for_the_machine_first_and_pool_waits_for_it() {
    let setup = of(
        Kind::RemoteDocker,
        &remote_facts(Connecting::default()),
        &chosen(),
    );
    assert_eq!(setup.at, Some(StepId::Connect));
    let connect = setup.step(StepId::Connect).unwrap();
    assert_eq!(connect.state, StepState::Current);
    assert_eq!(connect.next.as_ref().unwrap().id, ActionId::ConnectMachine);
    assert!(connect.findings[0].contains("opens no port there"));
    assert_eq!(setup.step(StepId::Pool).unwrap().state, StepState::Later);
}

#[test]
fn a_machine_picked_is_contacted_only_when_the_person_presses_connect_to_it() {
    let connect = connect_of(&remote_facts(routed("ssh://me@build-box")));
    let next = connect.next.unwrap();
    assert_eq!(next.id, ActionId::ConnectMachine);
    assert_eq!(next.label, "Connect to build-box");
    assert!(next.does.contains("keeps nothing"), "{}", next.does);
    assert_eq!(connect.machine.unwrap().host.as_deref(), Some("build-box"));
}

#[test]
fn plain_tcp_is_refused_on_an_attention_card_with_ssh_as_the_way_on() {
    let connect = connect_of(&remote_facts(routed("tcp://build-box:2375")));
    assert_eq!(connect.state, StepState::Attention);
    assert_eq!(connect.cause, Some("plain-tcp"));
    assert_eq!(connect.next.as_ref().unwrap().id, ActionId::UseSshInstead);
    assert!(connect.findings[0].contains("anyone who can reach that port"));
}

#[test]
fn a_key_met_is_shown_by_its_fingerprint_with_trust_this_machine_as_the_next() {
    let connect = connect_of(&remote_facts(Connecting {
        met: Some(Met {
            key: key(KEY),
            known: None,
        }),
        ..routed("ssh://me@build-box")
    }));
    assert_eq!(connect.state, StepState::Current);
    assert_eq!(connect.next.as_ref().unwrap().id, ActionId::TrustMachine);
    assert_eq!(connect.next.as_ref().unwrap().label, "Trust this machine");
    let machine = connect.machine.unwrap();
    assert_eq!(machine.key.unwrap().fingerprint, FINGERPRINT);
    assert_eq!(machine.known, None);
    assert!(connect.findings[0].contains("compare the two"));
}

#[test]
fn a_key_the_persons_own_ssh_knows_is_said_so_and_another_one_is_warned_of() {
    let met = |known: &str| {
        connect_of(&remote_facts(Connecting {
            met: Some(Met {
                key: key(KEY),
                known: Some(key(known)),
            }),
            ..routed("ssh://me@build-box")
        }))
    };
    let same = met(KEY);
    assert_eq!(same.machine.unwrap().known, Some(true));
    assert!(same.findings[0].contains("already knows it"));
    let other = met(OTHER);
    assert_eq!(other.machine.unwrap().known, Some(false));
    assert!(other.findings[0].contains("knows a different one"));
}

#[test]
fn a_trusted_machine_is_done_with_its_key_and_pool_makes_its_runner_there() {
    let facts = remote_facts(Connecting {
        trusted: Some(Pin {
            host_key: key(KEY),
            engine: crate::sshroute::EnginePin::Docker("ABCD".to_owned()),
        }),
        ..routed("ssh://me@build-box/var/run/docker.sock")
    });
    let setup = of(Kind::RemoteDocker, &facts, &chosen());
    let connect = setup.step(StepId::Connect).unwrap();
    assert_eq!(connect.state, StepState::Done);
    assert_eq!(
        connect.line.as_deref(),
        Some(format!("build-box, key {FINGERPRINT}").as_str())
    );
    assert_eq!(setup.at, Some(StepId::Pool));
    let pool = setup.step(StepId::Pool).unwrap();
    assert!(
        pool.next
            .as_ref()
            .unwrap()
            .does
            .contains("in Docker on build-box"),
        "{:?}",
        pool.next
    );
}

#[test]
fn each_finding_is_said_in_its_own_words_with_its_cause() {
    let cases = [
        (
            Finding::NoSuchName {
                host: "build-box".into(),
            },
            "no-such-name",
            "purlis couldn't find a machine called build-box. Check the name, or pick it from \
             your SSH hosts.",
            ActionId::ChangeMachine,
        ),
        (
            Finding::NoAnswer {
                host: "build-box".into(),
                port: 22,
            },
            "no-answer",
            "build-box didn't answer on port 22.",
            ActionId::CheckAgain,
        ),
        (
            Finding::SignInRefused {
                host: "build-box".into(),
            },
            "sign-in-refused",
            "build-box didn't accept your sign-in. purlis signs in with your SSH agent and \
             never asks for a password. Add your key to the agent with `ssh-add`, then check again.",
            ActionId::CheckAgain,
        ),
        (
            Finding::NoEngine {
                host: "build-box".into(),
                user: "me".into(),
            },
            "no-engine",
            "build-box has no Docker running that me can use.",
            ActionId::CheckAgain,
        ),
        (
            Finding::NotAllowed {
                host: "build-box".into(),
                user: "me".into(),
                socket: "/var/run/docker.sock".into(),
            },
            "not-allowed",
            "me on build-box isn't allowed to use Docker. On build-box, add me to the \
             docker group, or use rootless Docker or Podman",
            ActionId::CheckAgain,
        ),
        (
            Finding::EngineChanged {
                host: "build-box".into(),
                pinned: "OLD".into(),
                now: "NEW".into(),
                podman: false,
            },
            "pool-engine-changed",
            "The Docker engine on build-box isn't the one you trusted",
            ActionId::CheckAndTrustAgain,
        ),
        (
            Finding::EngineChanged {
                host: "build-box".into(),
                pinned: "OLD".into(),
                now: "NEW".into(),
                podman: true,
            },
            "pool-engine-changed",
            "Podman on build-box isn't the one you trusted: its store is somewhere else now",
            ActionId::CheckAndTrustAgain,
        ),
        (
            Finding::NeedsPodman {
                host: "build-box".into(),
            },
            "needs-podman",
            "build-box runs Podman, and purlis reaches Podman with Podman's own client",
            ActionId::CheckAgain,
        ),
    ];
    for (finding, cause, words, next) in cases {
        let step = found(finding.clone());
        assert_eq!(step.state, StepState::Attention, "{finding:?}");
        assert_eq!(step.cause, Some(cause));
        assert!(
            step.findings[0].starts_with(words),
            "{finding:?}: {}",
            step.findings[0]
        );
        assert_eq!(step.next.as_ref().unwrap().id, next, "{finding:?}");
        assert_eq!(said(&finding), step.findings[0]);
    }
}

#[test]
fn an_ssh_server_that_refuses_forwarding_is_named_with_what_to_ask_its_admin() {
    let step = found(Finding::ForwardingRefused {
        host: "build-box".into(),
    });
    assert_eq!(step.state, StepState::Attention);
    assert_eq!(step.cause, Some("forwarding-refused"));
    assert_eq!(
        step.findings,
        [
            "The SSH server on build-box doesn't allow forwarding, and purlis reaches Docker \
             there by forwarding its socket. Ask its admin to allow it for your account \
             (`AllowTcpForwarding`, `AllowStreamLocalForwarding` and `DisableForwarding` in \
             sshd_config, or `restrict` and `no-port-forwarding` on your key), then check \
             again."
        ]
    );
    assert_eq!(step.next.unwrap().id, ActionId::CheckAgain);
}

#[test]
fn a_changed_key_shows_both_fingerprints_and_offers_to_check_and_trust_again() {
    let step = found(Finding::KeyChanged {
        host: "build-box".into(),
        pinned: key(KEY),
        now: key(OTHER),
    });
    assert_eq!(step.cause, Some("pool-pin-changed"));
    assert!(step.findings[0].contains(FINGERPRINT));
    assert!(step.findings[0].contains(&key(OTHER).fingerprint()));
    assert_eq!(step.next.unwrap().label, "Check and trust again");
    assert_eq!(
        step.machine.unwrap().now.unwrap().fingerprint,
        key(OTHER).fingerprint()
    );
}

#[test]
fn where_offers_the_other_kind_beside_its_own() {
    let docker = of(
        Kind::Docker,
        &Facts {
            connect: None,
            ..remote_facts(Connecting::default())
        },
        &Choices::default(),
    );
    let other = docker.step(StepId::Where).unwrap().other.clone().unwrap();
    assert_eq!(other.id, ActionId::UseRemoteDocker);
    assert_eq!(other.label, "Set up Docker on another machine");
    let remote = of(
        Kind::RemoteDocker,
        &remote_facts(Connecting::default()),
        &chosen(),
    );
    let where_ = remote.step(StepId::Where).unwrap();
    assert_eq!(where_.state, StepState::Done);
    assert_eq!(where_.line.as_deref(), Some("Docker on another machine"));
}

#[test]
fn where_offers_another_machine_and_a_cluster_each_by_its_own_way_and_neither_twice() {
    let with_a_cluster = || super::super::cluster::ClusterFacts {
        contexts: vec!["team".to_owned()],
        ..super::super::cluster::ClusterFacts::default()
    };
    // This machine's Docker: another machine beside it, and the team's cluster instead.
    let docker = of(
        Kind::Docker,
        &Facts {
            connect: None,
            cluster: with_a_cluster(),
            ..remote_facts(Connecting::default())
        },
        &Choices::default(),
    );
    let where_ = docker.step(StepId::Where).unwrap();
    assert_eq!(
        where_.other.as_ref().map(|one| one.id),
        Some(ActionId::UseRemoteDocker)
    );
    assert_eq!(
        where_.also.as_ref().map(|one| one.id),
        Some(ActionId::UseCluster)
    );
    // Another machine's: back to this machine's Docker, and no cluster beside it.
    let remote = of(
        Kind::RemoteDocker,
        &Facts {
            cluster: with_a_cluster(),
            ..remote_facts(routed("ssh://sam@build-box"))
        },
        &Choices {
            kind: Some(Kind::RemoteDocker),
            ..Choices::default()
        },
    );
    let where_ = remote.step(StepId::Where).unwrap();
    assert!(where_.also.is_none(), "{where_:?}");
    // A cluster's: back to Docker, and no other machine beside it.
    let cluster = of(
        Kind::Cluster,
        &Facts {
            connect: None,
            cluster: with_a_cluster(),
            ..remote_facts(Connecting::default())
        },
        &Choices::default(),
    );
    let where_ = cluster.step(StepId::Where).unwrap();
    assert!(where_.other.is_none(), "{where_:?}");
}

#[test]
fn a_pool_on_another_machines_sign_ins_are_a_docker_pools() {
    let facts = Facts {
        pool: Some(PoolFacts {
            name: "build-box".into(),
            runner: Runner::Running,
            sign_ins: SignIns::Read(Vec::new()),
            cluster: None,
        }),
        ..remote_facts(Connecting {
            trusted: Some(Pin {
                host_key: key(KEY),
                engine: crate::sshroute::EnginePin::Docker("ABCD".to_owned()),
            }),
            ..routed("ssh://me@build-box/var/run/docker.sock")
        })
    };
    let setup = of(Kind::RemoteDocker, &facts, &chosen());
    let sign_ins = setup.step(StepId::Signins).unwrap();
    assert_eq!(sign_ins.state, StepState::Current);
    assert_eq!(sign_ins.next.as_ref().unwrap().id, ActionId::ChooseProfiles);
}

#[test]
fn connects_by_hand_print_the_key_on_the_machine_and_ask_docker_there_for_its_id() {
    let see = super::by_hand(Some("ssh://me@build-box"));
    assert!(
        see.text
            .contains("ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub")
    );
    assert!(
        see.text
            .contains("docker -H ssh://me@build-box info --format '{{.ID}}'")
    );
}

#[test]
fn the_machine_connect_reads_is_kept_and_a_pin_only_once_trusted() {
    let config = tempfile::tempdir().unwrap();
    choose_machine(config.path(), Some("ssh://me@build-box"), None).unwrap();
    let kept = read_choices(config.path()).unwrap();
    assert_eq!(kept.kind, Some(Kind::RemoteDocker));
    assert_eq!(
        kept.connect,
        Some(Chosen {
            identity: None,
            route: "ssh://me@build-box".into(),
            trusted: None,
            tls: None,
            trusted_tls: None,
            docker_context: None,
            owned: false,
        })
    );
    let remote = crate::runners::Remote {
        route: Route::parse("ssh://me@build-box/var/run/docker.sock").unwrap(),
        pin: Pin {
            host_key: key(KEY),
            engine: crate::sshroute::EnginePin::Docker("ABCD".to_owned()),
        },
    };
    trusted(config.path(), &remote).unwrap();
    let kept = read_choices(config.path()).unwrap();
    assert_eq!(kept.connect.unwrap().trusted, Some(remote.pin));
    let file = std::fs::read_to_string(
        crate::machine::dir(config.path()).join(super::super::CHOICES_FILE),
    )
    .unwrap();
    let file: serde_json::Value = serde_json::from_str(&file).unwrap();
    assert_eq!(
        file["connect"],
        serde_json::json!({
            "route": "ssh://me@build-box/var/run/docker.sock",
            "trusted": { "hostKey": KEY, "engine": "ABCD" },
        })
    );
    // Another kind on Where forgets the machine.
    choose_kind(config.path(), Kind::Docker).unwrap();
    assert_eq!(read_choices(config.path()).unwrap().connect, None);
}

/// A machine picked from a Podman connection keeps its key file, and a Podman's pin, socket and
/// graph root, is kept as trusted and read back whole (#6269), as the Pool step makes the pool
/// with it.
#[test]
fn a_podman_connections_key_file_and_a_podmans_pin_are_kept_and_read_back() {
    let config = tempfile::tempdir().unwrap();
    let identity = std::path::Path::new("/home/me/.ssh/id_ed25519");
    let route = "ssh://me@build-box/run/user/1000/podman/podman.sock";
    choose_machine(config.path(), Some(route), Some(identity)).unwrap();
    let kept = read_choices(config.path()).unwrap().connect.unwrap();
    assert_eq!(kept.identity.as_deref(), Some(identity));
    assert_eq!(kept.route_read().unwrap().identity(), Some(identity));
    let remote = crate::runners::Remote {
        route: kept.route_read().unwrap(),
        pin: Pin {
            host_key: key(KEY),
            engine: crate::sshroute::EnginePin::Podman {
                socket: "/run/user/1000/podman/podman.sock".into(),
                graph_root: "/home/me/.local/share/containers/storage".into(),
            },
        },
    };
    crate::poolsetup::trusted(config.path(), &remote).unwrap();
    let kept = read_choices(config.path()).unwrap().connect.unwrap();
    assert_eq!(kept.trusted.as_ref(), Some(&remote.pin));
    assert_eq!(kept.route_read().unwrap(), remote.route);
    // A key file no route can sign in with is never kept.
    assert!(
        choose_machine(
            config.path(),
            Some(route),
            Some(std::path::Path::new("keys/%h"))
        )
        .is_err()
    );
}

// ---------------------------------------------------------------------------------------------
// Over Docker's TLS API (#6267).

fn files() -> crate::tlsroute::Files {
    crate::tlsroute::Files::new(
        "/home/me/.docker/ca.pem".into(),
        "/home/me/.docker/cert.pem".into(),
        crate::tlsroute::KeyFrom::File("/home/me/.docker/key.pem".into()),
    )
    .unwrap()
}

fn ca() -> crate::tlsroute::Fingerprint {
    crate::tlsroute::Fingerprint::of_der(b"the CA")
}

fn over_tls() -> Connecting {
    Connecting {
        tls: Some(files()),
        ..routed("tcp://build-box:2376")
    }
}

fn tls_found(finding: Finding) -> Step {
    connect_of(&remote_facts(Connecting {
        finding: Some(finding),
        ..over_tls()
    }))
}

#[test]
fn a_tcp_machine_without_tls_files_is_refused_as_plain_tcp_naming_the_way_to_turn_tls_on() {
    let connect = connect_of(&remote_facts(routed("tcp://build-box:2376")));
    assert_eq!(connect.state, StepState::Attention);
    assert_eq!(connect.cause, Some("plain-tcp"));
    assert_eq!(connect.next.as_ref().unwrap().id, ActionId::UseSshInstead);
    assert!(connect.findings[0].contains("anyone who can reach that port"));
    assert!(connect.findings[0].contains("under Options"));
    assert_eq!(connect.machine.unwrap().over, "tls");
}

#[test]
fn a_tcp_machine_with_its_files_named_is_connected_to_then_its_ca_shown_to_trust() {
    let connect = connect_of(&remote_facts(over_tls()));
    assert_eq!(connect.state, StepState::Current);
    let next = connect.next.as_ref().unwrap();
    assert_eq!(next.id, ActionId::ConnectMachine);
    assert_eq!(next.label, "Connect to build-box");
    let machine = connect.machine.unwrap();
    assert_eq!(machine.tls.unwrap().cert, "/home/me/.docker/cert.pem");

    let connect = connect_of(&remote_facts(Connecting {
        met_ca: Some(CaMet {
            fingerprint: ca(),
            subject: Some("CN=docker-ca".to_owned()),
            file: "/home/me/.docker/ca.pem".to_owned(),
        }),
        ..over_tls()
    }));
    assert_eq!(connect.next.as_ref().unwrap().id, ActionId::TrustMachine);
    assert!(connect.findings[0].contains("compare the two fingerprints"));
    let shown = connect.machine.unwrap().ca.unwrap();
    assert_eq!(shown.fingerprint, ca().shown());
    assert_eq!(shown.subject.as_deref(), Some("CN=docker-ca"));
}

#[test]
fn a_trusted_tcp_machine_is_done_with_its_ca_and_pool_goes_on_there() {
    let setup = of(
        Kind::RemoteDocker,
        &remote_facts(Connecting {
            trusted_tls: Some(crate::tlsroute::Pin {
                ca: ca(),
                engine: "ABCD".to_owned(),
            }),
            ..over_tls()
        }),
        &chosen(),
    );
    let connect = setup.step(StepId::Connect).unwrap();
    assert_eq!(connect.state, StepState::Done);
    assert_eq!(
        connect.line.as_deref(),
        Some(format!("build-box, CA {}", ca().shown()).as_str())
    );
    assert_eq!(setup.at, Some(StepId::Pool));
    let pool = setup.step(StepId::Pool).unwrap();
    assert!(
        pool.next.as_ref().unwrap().does.contains("build-box"),
        "the pool is made on the other machine"
    );
}

#[test]
fn a_ca_file_that_no_longer_matches_shows_both_and_offers_to_check_and_trust_again() {
    let step = tls_found(Finding::CaChanged {
        host: "build-box".into(),
        pinned: ca().shown(),
        now: "AA:BB".into(),
        file: "/home/me/.docker/ca.pem".into(),
    });
    assert_eq!(step.state, StepState::Attention);
    assert_eq!(step.cause, Some("pool-pin-changed"));
    assert!(step.findings[0].contains("/home/me/.docker/ca.pem"));
    assert!(step.findings[0].contains(&ca().shown()));
    assert_eq!(step.next.unwrap().label, "Check and trust again");
    assert_eq!(step.machine.unwrap().ca_now.as_deref(), Some("AA:BB"));
}

#[test]
fn each_tls_finding_says_which_file_to_check() {
    let host = || "build-box".to_owned();
    for (finding, cause, file) in [
        (
            Finding::NotSignedByCa {
                host: host(),
                ca: "/home/me/.docker/ca.pem".into(),
            },
            "tls-not-signed",
            "/home/me/.docker/ca.pem",
        ),
        (
            Finding::ServerExpired { host: host() },
            "tls-expired",
            "--tlscert names there",
        ),
        (
            Finding::WrongName {
                host: host(),
                name: "docker, localhost".into(),
            },
            "tls-wrong-name",
            "--tlscert names on build-box",
        ),
        (
            Finding::ClientRefused {
                host: host(),
                cert: "/home/me/.docker/cert.pem".into(),
            },
            "tls-client-refused",
            "/home/me/.docker/cert.pem",
        ),
        (
            Finding::ClientExpired {
                host: host(),
                cert: "/home/me/.docker/cert.pem".into(),
            },
            "tls-client-expired",
            "/home/me/.docker/cert.pem",
        ),
        (
            Finding::TlsFile {
                host: host(),
                file: "/home/me/.docker/key.pem".into(),
                why: "isn't there".into(),
            },
            "tls-file",
            "/home/me/.docker/key.pem isn't there",
        ),
    ] {
        let step = tls_found(finding.clone());
        assert_eq!(step.state, StepState::Attention, "{finding:?}");
        assert_eq!(step.cause, Some(cause), "{finding:?}");
        assert!(step.findings[0].contains(file), "{}", step.findings[0]);
        assert!(step.next.is_some(), "{finding:?} offers a way on");
        // The same words in the window and in `purlis pool`.
        assert_eq!(step.findings[0], said(&finding));
    }
    let named = tls_found(Finding::WrongName {
        host: host(),
        name: "docker, localhost".into(),
    });
    assert!(named.findings[0].contains("another name (docker, localhost)"));
    let expired = said(&Finding::ServerExpired { host: host() });
    assert!(expired.contains("build-box's certificate has expired"));
    let wrong_ca = said(&Finding::NotSignedByCa {
        host: host(),
        ca: "/ca.pem".into(),
    });
    assert!(wrong_ca.contains("isn't signed by the CA you chose"));
}

#[test]
fn see_what_this_does_over_tls_prints_the_cas_fingerprint_there_and_names_the_files() {
    let see = by_hand_tls("tcp://build-box:2376", Some(&files()));
    assert!(
        see.text
            .contains("openssl x509 -noout -fingerprint -sha256")
    );
    assert!(see.text.contains(
        "docker -H tcp://build-box:2376 --tlsverify --tlscacert /home/me/.docker/ca.pem \
         --tlscert /home/me/.docker/cert.pem --tlskey /home/me/.docker/key.pem info"
    ));
}

#[test]
fn the_tls_files_are_kept_with_a_tcp_machine_and_its_ca_pin_only_once_trusted() {
    let config = tempfile::tempdir().unwrap();
    assert!(
        choose_tls_files(config.path(), Some(files())).is_err(),
        "no machine yet"
    );
    choose_machine(config.path(), Some("tcp://build-box:2376"), None).unwrap();
    choose_tls_files(config.path(), Some(files())).unwrap();
    // Another tcp:// machine typed keeps the files; an ssh:// one doesn't take them.
    choose_machine(config.path(), Some("tcp://other-box"), None).unwrap();
    assert_eq!(
        read_choices(config.path()).unwrap().connect.unwrap().tls,
        Some(files())
    );
    choose_machine(config.path(), Some("ssh://me@box"), None).unwrap();
    assert_eq!(
        read_choices(config.path()).unwrap().connect.unwrap().tls,
        None
    );
    assert!(choose_tls_files(config.path(), Some(files())).is_err());

    let remote = crate::tlsroute::Remote {
        route: crate::tlsroute::Route::parse("tcp://build-box").unwrap(),
        files: crate::tlsroute::Files::new(
            "/home/me/.docker/ca.pem".into(),
            "/home/me/.docker/cert.pem".into(),
            crate::tlsroute::KeyFrom::Vault {
                plane: "/home/me/plane".into(),
                vault: "docker-box".into(),
                key: "TLS_KEY".into(),
            },
        )
        .unwrap(),
        pin: crate::tlsroute::Pin {
            ca: ca(),
            engine: "ABCD".to_owned(),
        },
    };
    trusted_tls(config.path(), &remote).unwrap();
    let file = std::fs::read_to_string(
        crate::machine::dir(config.path()).join(super::super::CHOICES_FILE),
    )
    .unwrap();
    let file: serde_json::Value = serde_json::from_str(&file).unwrap();
    assert_eq!(
        file["connect"],
        serde_json::json!({
            "route": "tcp://build-box:2376",
            "tls": {
                "ca": "/home/me/.docker/ca.pem",
                "cert": "/home/me/.docker/cert.pem",
                "keyIn": { "plane": "/home/me/plane", "vault": "docker-box", "key": "TLS_KEY" },
            },
            "trusted": { "ca": ca().spelled(), "engine": "ABCD" },
        })
    );
    let kept = read_choices(config.path()).unwrap().connect.unwrap();
    assert_eq!(kept.trusted_tls, Some(remote.pin.clone()));
    assert_eq!(kept.trusted, None);
    // Files named again forget the trust: it was of the files as they were.
    choose_tls_files(config.path(), Some(files())).unwrap();
    assert_eq!(
        read_choices(config.path())
            .unwrap()
            .connect
            .unwrap()
            .trusted_tls,
        None
    );
}

/// **Connect and Trust this machine refuse TLS files purlis can't hold from chats** (#6267):
/// a hard-linked key, and files in a folder chats write, before anything is read or met.
#[cfg(unix)]
#[test]
fn connect_and_trust_over_tls_refuse_files_a_chat_could_reach_another_way() {
    let at = tempfile::tempdir().unwrap();
    for name in ["ca.pem", "cert.pem", "key.pem"] {
        std::fs::write(at.path().join(name), "not read").unwrap();
    }
    let files = crate::tlsroute::Files::new(
        at.path().join("ca.pem"),
        at.path().join("cert.pem"),
        crate::tlsroute::KeyFrom::File(at.path().join("key.pem")),
    )
    .unwrap();
    let route = "tcp://box:2376";
    let fingerprint = format!("sha256:{}", "ab".repeat(32));
    let docker = crate::dockerpool::Docker::at("/nonexistent/docker");
    let refused = |met: Result<(), Finding>, file: &str, why: &str| match met {
        Err(Finding::TlsFile {
            file: named,
            why: said,
            ..
        }) => {
            assert!(named.ends_with(file), "{named}");
            assert!(said.contains(why), "{said}");
        }
        other => panic!("{file} was taken: {other:?}"),
    };
    let both = |chats_write: &[std::path::PathBuf]| {
        (
            meet_tls_with(route, &files, chats_write).map(drop),
            trust_tls_with(&docker, route, &files, &fingerprint, chats_write).map(drop),
        )
    };

    // In a folder chats write.
    let (met, trusted) = both(&[at.path().to_path_buf()]);
    refused(met, "ca.pem", "chats write");
    refused(trusted, "ca.pem", "chats write");

    // A key with another name on disk.
    let other = tempfile::tempdir().unwrap();
    std::fs::hard_link(at.path().join("key.pem"), other.path().join("k.pem")).unwrap();
    let (met, trusted) = both(&[]);
    refused(met, "key.pem", "another name");
    refused(trusted, "key.pem", "another name");
}

// ---------------------------------------------------------------------------------------------
// From a Docker context (#6268).

fn context(
    name: &str,
    endpoint: crate::dockercontext::Endpoint,
) -> crate::dockercontext::DockerContext {
    crate::dockercontext::DockerContext {
        name: name.to_owned(),
        current: false,
        endpoint,
    }
}

fn ssh_context() -> crate::dockercontext::DockerContext {
    context(
        "build-box",
        crate::dockercontext::Endpoint::Ssh(Route::parse("ssh://me@build-box").unwrap()),
    )
}

fn tls_context() -> crate::dockercontext::DockerContext {
    context(
        "lab",
        crate::dockercontext::Endpoint::Tls {
            route: crate::tlsroute::Route::parse("tcp://lab.example:2376").unwrap(),
            files: files(),
        },
    )
}

/// An ssh:// context picked is its route, read once, and its name is kept beside it through the
/// trust, for doctor alone; a machine typed after forgets the name.
#[test]
fn an_ssh_context_picked_keeps_its_route_and_its_name_through_the_trust() {
    let config = tempfile::tempdir().unwrap();
    choose_docker_context(config.path(), &ssh_context()).unwrap();
    let kept = read_choices(config.path()).unwrap();
    assert_eq!(kept.kind, Some(Kind::RemoteDocker));
    let chosen = kept.connect.unwrap();
    assert_eq!(chosen.route, "ssh://me@build-box");
    assert_eq!(chosen.docker_context.as_deref(), Some("build-box"));
    let remote = crate::runners::Remote {
        route: Route::parse("ssh://me@build-box/var/run/docker.sock").unwrap(),
        pin: Pin {
            host_key: key(KEY),
            engine: crate::sshroute::EnginePin::Docker("ABCD".to_owned()),
        },
    };
    trusted(config.path(), &remote).unwrap();
    let file = std::fs::read_to_string(
        crate::machine::dir(config.path()).join(super::super::CHOICES_FILE),
    )
    .unwrap();
    let file: serde_json::Value = serde_json::from_str(&file).unwrap();
    assert_eq!(file["connect"]["dockerContext"], "build-box");
    assert_eq!(
        read_choices(config.path())
            .unwrap()
            .connect
            .unwrap()
            .docker_context
            .as_deref(),
        Some("build-box")
    );
    // A machine typed is not the context's.
    choose_machine(config.path(), Some("ssh://me@other-box"), None).unwrap();
    assert_eq!(
        read_choices(config.path())
            .unwrap()
            .connect
            .unwrap()
            .docker_context,
        None
    );
}

/// A TLS context picked is its tcp:// route with its own TLS material's paths, and the name
/// stays through the trust of its CA.
#[test]
fn a_tls_context_picked_keeps_its_route_its_tls_files_and_its_name_through_the_trust() {
    let config = tempfile::tempdir().unwrap();
    choose_docker_context(config.path(), &tls_context()).unwrap();
    let chosen = read_choices(config.path()).unwrap().connect.unwrap();
    assert_eq!(chosen.route, "tcp://lab.example:2376");
    assert_eq!(chosen.tls, Some(files()));
    assert_eq!(chosen.docker_context.as_deref(), Some("lab"));
    let remote = crate::tlsroute::Remote {
        route: crate::tlsroute::Route::parse("tcp://lab.example:2376").unwrap(),
        files: files(),
        pin: crate::tlsroute::Pin {
            ca: ca(),
            engine: "ABCD".to_owned(),
        },
    };
    trusted_tls(config.path(), &remote).unwrap();
    let chosen = read_choices(config.path()).unwrap().connect.unwrap();
    assert_eq!(chosen.trusted_tls, Some(remote.pin));
    assert_eq!(chosen.docker_context.as_deref(), Some("lab"));
    // The same files kept again are still the context's; others are the person's own.
    choose_tls_files(config.path(), Some(files())).unwrap();
    assert_eq!(
        read_choices(config.path())
            .unwrap()
            .connect
            .unwrap()
            .docker_context
            .as_deref(),
        Some("lab")
    );
    let mine = crate::tlsroute::Files::new(
        "/home/me/certs/ca.pem".into(),
        "/home/me/certs/cert.pem".into(),
        crate::tlsroute::KeyFrom::File("/home/me/certs/key.pem".into()),
    )
    .unwrap();
    choose_tls_files(config.path(), Some(mine)).unwrap();
    assert_eq!(
        read_choices(config.path())
            .unwrap()
            .connect
            .unwrap()
            .docker_context,
        None
    );
}

/// A context on this machine's own socket is the local kind; one refused is refused in its own
/// words, and keeps nothing.
#[test]
fn a_local_socket_context_is_the_local_kind_and_a_refused_one_keeps_nothing() {
    let config = tempfile::tempdir().unwrap();
    choose_docker_context(
        config.path(),
        &context("desktop-linux", crate::dockercontext::Endpoint::Here),
    )
    .unwrap();
    let kept = read_choices(config.path()).unwrap();
    assert_eq!(kept.kind, Some(Kind::Docker));
    assert_eq!(kept.connect, None);
    let refused = choose_docker_context(
        config.path(),
        &context(
            "open",
            crate::dockercontext::Endpoint::Refused(crate::sshroute::PLAIN_TCP.to_owned()),
        ),
    );
    match refused {
        Err(Refused::Other(why)) => assert_eq!(why, crate::sshroute::PLAIN_TCP),
        other => panic!("taken: {other:?}"),
    }
    assert_eq!(
        read_choices(config.path()).unwrap().kind,
        Some(Kind::Docker)
    );
}

/// Doctor's check of the context (ADR 0123 §7): a pool whose context now names another route
/// says so in the ADR's words, offers Connect again and keeping the pool as it is, and changes
/// nothing the pool runs on.
#[test]
fn a_context_that_now_names_another_route_is_said_in_adr_0123s_words_with_two_ways_on() {
    let trusted_pin = Pin {
        host_key: key(KEY),
        engine: crate::sshroute::EnginePin::Docker("ABCD".to_owned()),
    };
    let connect = connect_of(&remote_facts(Connecting {
        trusted: Some(trusted_pin.clone()),
        docker_context: Some("build-box".to_owned()),
        context_now: Some("ssh://me@new-box".to_owned()),
        ..routed("ssh://me@build-box/var/run/docker.sock")
    }));
    assert_eq!(connect.state, StepState::Attention);
    assert_eq!(connect.cause, Some("context-moved"));
    assert_eq!(
        connect.findings,
        [
            "Your context build-box now points somewhere else than this pool. Connect again to \
          follow it, or keep the pool as it is."
        ]
    );
    assert_eq!(connect.next.as_ref().unwrap().id, ActionId::ConnectAgain);
    assert_eq!(connect.other.as_ref().unwrap().id, ActionId::KeepPoolAsItIs);
    assert_eq!(
        connect.prefill,
        Some(Prefill {
            route: "ssh://me@new-box".to_owned(),
            context: Some("build-box".to_owned()),
        })
    );
    // The pool's own machine is still the one shown.
    let machine = connect.machine.unwrap();
    assert_eq!(
        machine.route.as_deref(),
        Some("ssh://me@build-box/var/run/docker.sock")
    );
    assert_eq!(machine.context.as_deref(), Some("build-box"));
    // A context that still names the route says nothing.
    let still = connect_of(&remote_facts(Connecting {
        trusted: Some(trusted_pin),
        docker_context: Some("build-box".to_owned()),
        ..routed("ssh://me@build-box/var/run/docker.sock")
    }));
    assert_eq!(still.state, StepState::Done);
    assert_eq!(still.other, None);
}

/// The local kind's engine elsewhere offers Set up Docker on another machine beside its own
/// words, prefilled from DOCKER_HOST or the current context where that is an SSH or TLS address.
#[test]
fn an_engine_elsewhere_offers_docker_on_another_machine_prefilled_with_its_address() {
    let prefill = Prefill {
        route: "ssh://me@build-box".to_owned(),
        context: None,
    };
    let setup = of(
        Kind::Docker,
        &Facts {
            engine: Engine::Elsewhere,
            elsewhere: Some(prefill.clone()),
            connect: None,
            ..remote_facts(Connecting::default())
        },
        &Choices::default(),
    );
    let where_ = setup.step(StepId::Where).unwrap();
    assert_eq!(where_.state, StepState::Attention);
    assert_eq!(where_.next.as_ref().unwrap().id, ActionId::UseLocalDocker);
    let other = where_.other.as_ref().unwrap();
    assert_eq!(other.id, ActionId::UseRemoteDocker);
    assert_eq!(other.label, "Set up Docker on another machine");
    assert_eq!(where_.prefill, Some(prefill));
}

#[test]
fn the_prefill_is_docker_host_where_it_is_remote_and_else_the_current_context() {
    let mut current = ssh_context();
    current.current = true;
    let contexts = [tls_context(), current];
    assert_eq!(
        prefill_of(Some("ssh://me@host-box"), &contexts),
        Some(Prefill {
            route: "ssh://me@host-box".to_owned(),
            context: None,
        })
    );
    assert_eq!(
        prefill_of(None, &contexts),
        Some(Prefill {
            route: "ssh://me@build-box".to_owned(),
            context: Some("build-box".to_owned()),
        })
    );
    assert_eq!(
        prefill_of(Some("unix:///var/run/docker.sock"), &contexts[..1]),
        None,
        "a socket here, and no current context"
    );
    assert_eq!(
        prefill_of(Some("tcp://lab.example:2376"), &[])
            .unwrap()
            .route,
        "tcp://lab.example:2376"
    );
}

fn pin_of(text: &str) -> Pin {
    Pin {
        host_key: key(text),
        engine: crate::sshroute::EnginePin::Docker("OLD:ENGINE".into()),
    }
}

/// **Check and trust again, over SSH** (#6270): a made pool's machine met again shows the key it
/// shows now beside the one trusted, for the person to check, and offers Trust this machine.
#[test]
fn a_machine_met_again_shows_the_key_it_shows_now_beside_the_one_trusted() {
    let step = connect_of(&remote_facts(Connecting {
        trusted: Some(pin_of(KEY)),
        met: Some(Met {
            key: key(OTHER),
            known: None,
        }),
        ..routed("ssh://me@build-box")
    }));
    assert_eq!(step.next.as_ref().unwrap().id, ActionId::TrustMachine);
    let machine = step.machine.unwrap();
    assert_eq!(machine.trusted.unwrap().fingerprint, FINGERPRINT);
    assert_eq!(machine.key.unwrap().fingerprint, key(OTHER).fingerprint());
    assert!(
        step.findings[0].contains("than the one you trusted"),
        "{:?}",
        step.findings
    );
}

/// Where only the engine changed, the key shown is the one trusted, and trusting reads the
/// engine there anew.
#[test]
fn a_machine_met_again_with_the_key_trusted_says_its_engine_is_trusted_anew() {
    let step = connect_of(&remote_facts(Connecting {
        trusted: Some(pin_of(KEY)),
        met: Some(Met {
            key: key(KEY),
            known: None,
        }),
        ..routed("ssh://me@build-box")
    }));
    assert_eq!(step.next.as_ref().unwrap().id, ActionId::TrustMachine);
    assert!(
        step.findings[0].contains("the key you trusted"),
        "{:?}",
        step.findings
    );
    assert!(
        step.next.unwrap().does.contains("engine"),
        "trusting keeps the engine there now"
    );
}

/// **Check and trust again, over TLS** (#6270): the CA the file holds now beside the one trusted.
#[test]
fn a_machine_over_tls_met_again_shows_the_ca_it_holds_now_beside_the_one_trusted() {
    let files = crate::tlsroute::Files::new(
        "/home/me/.docker/ca.pem".into(),
        "/home/me/.docker/cert.pem".into(),
        crate::tlsroute::KeyFrom::File("/home/me/.docker/key.pem".into()),
    )
    .unwrap();
    let trusted = crate::tlsroute::Fingerprint::of_der(b"old ca");
    let now = crate::tlsroute::Fingerprint::of_der(b"new ca");
    let step = connect_of(&remote_facts(Connecting {
        route: Some("tcp://build-box:2376".into()),
        tls: Some(files),
        trusted_tls: Some(crate::tlsroute::Pin {
            ca: trusted,
            engine: "OLD:ENGINE".into(),
        }),
        met_ca: Some(CaMet {
            fingerprint: now,
            subject: None,
            file: "/home/me/.docker/ca.pem".into(),
        }),
        ..Connecting::default()
    }));
    assert_eq!(step.next.as_ref().unwrap().id, ActionId::TrustMachine);
    let machine = step.machine.unwrap();
    assert_eq!(machine.trusted_ca, Some(trusted.shown()));
    assert_eq!(machine.ca.unwrap().fingerprint, now.shown());
    assert!(
        step.findings[0].contains("than the one you trusted"),
        "{:?}",
        step.findings
    );
}

/// **Connect's last answer, who can sign in to the machine, is kept only beside a trusted pin**
/// (#6271, ADR 0123 §5): not marked until the person says so, and trusting the machine again
/// forgets it, since it was the answer for the pin as it was.
#[test]
fn who_can_sign_in_is_kept_only_beside_a_trusted_pin_and_trusting_again_forgets_it() {
    let config = tempfile::tempdir().unwrap();
    choose_machine(config.path(), Some("ssh://me@build-box"), None).unwrap();
    assert!(
        choose_owned(config.path(), true).is_err(),
        "nothing is trusted yet"
    );
    let remote = crate::runners::Remote {
        route: Route::parse("ssh://me@build-box/var/run/docker.sock").unwrap(),
        pin: Pin {
            host_key: key(KEY),
            engine: crate::sshroute::EnginePin::Docker("ABCD".to_owned()),
        },
    };
    trusted(config.path(), &remote).unwrap();
    assert!(!read_choices(config.path()).unwrap().connect.unwrap().owned);
    choose_owned(config.path(), true).unwrap();
    assert!(read_choices(config.path()).unwrap().connect.unwrap().owned);
    let file = || -> serde_json::Value {
        serde_json::from_str(
            &std::fs::read_to_string(
                crate::machine::dir(config.path()).join(super::super::CHOICES_FILE),
            )
            .unwrap(),
        )
        .unwrap()
    };
    assert_eq!(file()["connect"]["owned"], serde_json::json!(true));
    choose_owned(config.path(), false).unwrap();
    assert!(file()["connect"].get("owned").is_none());
    choose_owned(config.path(), true).unwrap();
    trusted(config.path(), &remote).unwrap();
    assert!(!read_choices(config.path()).unwrap().connect.unwrap().owned);
}

/// **Connect shows whether the person said only they sign in there**, for the question to hold.
#[test]
fn a_trusted_machine_shows_who_the_person_said_can_sign_in_to_it() {
    let trusted = |owned: bool| {
        connect_of(&remote_facts(Connecting {
            trusted: Some(Pin {
                host_key: key(KEY),
                engine: crate::sshroute::EnginePin::Docker("ABCD".to_owned()),
            }),
            owned,
            ..routed("ssh://me@build-box/var/run/docker.sock")
        }))
    };
    assert!(trusted(true).machine.unwrap().owned);
    assert!(!trusted(false).machine.unwrap().owned);
    let tls = connect_of(&remote_facts(Connecting {
        trusted_tls: Some(crate::tlsroute::Pin {
            ca: ca(),
            engine: "ABCD".to_owned(),
        }),
        owned: true,
        ..over_tls()
    }));
    assert!(tls.machine.unwrap().owned);
}
