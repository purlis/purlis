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
