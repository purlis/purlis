// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::super::*;
use super::*;

use StepState::{Attention, Current, Done, Later};

fn on_a_cluster(cluster: ClusterFacts) -> Facts {
    Facts {
        engine: Engine::Missing,
        pool: None,
        identity: Identity::None,
        repos: true,
        cluster,
    }
}

fn contexts(names: &[&str], current: Option<&str>) -> ClusterFacts {
    ClusterFacts {
        contexts: names.iter().map(|name| (*name).to_owned()).collect(),
        current: current.map(str::to_owned),
        ..ClusterFacts::default()
    }
}

fn chose(context: &str) -> Choices {
    Choices {
        kind: Some(Kind::Cluster),
        context: Some(context.to_owned()),
        ..Choices::default()
    }
}

fn states(setup: &Setup) -> Vec<StepState> {
    setup.steps.iter().map(|step| step.state).collect()
}

/// Doctor's report on a cluster with nothing on it, as `docs/cluster-operator.md` shows it.
fn nothing_installed() -> doctor::Report {
    doctor::Report {
        holds: vec![
            "asked context kind-team, API server https://127.0.0.1:6443/".into(),
            "Kubernetes v1.36.4".into(),
            "you may apply the install's cluster-scoped definitions".into(),
        ],
        findings: vec![doctor::Finding {
            install: true,
            ..doctor::Finding::new(
                "upstream agent-sandbox v1.0.6: not installed",
                "apply it, step 1 of the install",
            )
        }],
    }
}

fn empty_cluster() -> Installed {
    Installed {
        definitions: false,
        key: false,
        operator: Operator::Absent,
        claims: Some((0, 0)),
    }
}

fn chosen(report: doctor::Report, installed: Installed) -> ClusterFacts {
    ClusterFacts {
        chosen: Some("kind-team".into()),
        doctor: Some(report),
        installed: Some(Ok(installed)),
        ..contexts(&["kind-team"], Some("kind-team"))
    }
}

#[test]
fn a_cluster_has_the_seven_steps_of_the_ruling_in_order() {
    let names: Vec<_> = Kind::Cluster.steps().iter().map(|s| s.name()).collect();
    assert_eq!(
        names,
        [
            "Where chats run",
            "Check the cluster",
            "Install",
            "Pool",
            "Sign-ins",
            "Agent identity",
            "Try it"
        ]
    );
}

#[test]
fn where_on_a_cluster_recommends_the_current_context_and_keeps_nothing_until_it_is_chosen() {
    let facts = on_a_cluster(contexts(&["kind-team", "prod"], Some("kind-team")));
    let setup = of(Kind::Cluster, &facts, &Choices::default());
    let place = setup.step(StepId::Where).unwrap();
    assert_eq!(place.state, Current);
    assert!(place.findings[0].contains("context kind-team"), "{place:?}");
    assert!(place.findings[0].contains("keeps none of it"), "{place:?}");
    assert_eq!(place.next.as_ref().unwrap().id, ActionId::UseCluster);
    assert_eq!(setup.contexts.recommended.as_deref(), Some("kind-team"));
    assert_eq!(setup.contexts.chosen, None);
    // Docker is the choice purlis didn't recommend.
    assert_eq!(place.also.as_ref().unwrap().id, ActionId::UseDocker);
}

#[test]
fn where_with_no_context_says_there_is_no_cluster_and_offers_docker() {
    let setup = of(
        Kind::Cluster,
        &on_a_cluster(contexts(&[], None)),
        &Choices::default(),
    );
    let place = setup.step(StepId::Where).unwrap();
    assert_eq!(place.state, Attention);
    assert!(place.findings[0].contains("names no cluster"), "{place:?}");
    assert_eq!(place.next.as_ref().unwrap().id, ActionId::UseDocker);
}

#[test]
fn where_without_kubectl_opens_its_install_page() {
    let facts = on_a_cluster(ClusterFacts {
        no_kubectl: Some("purlis found no kubectl on this machine".into()),
        ..ClusterFacts::default()
    });
    let setup = of(Kind::Cluster, &facts, &Choices::default());
    let next = setup.step(StepId::Where).unwrap().next.clone().unwrap();
    assert_eq!(next.id, ActionId::GetKubectl);
    assert_eq!(next.url.as_deref(), Some(KUBECTL_INSTALL_PAGE));
}

#[test]
fn a_docker_setup_offers_the_cluster_its_kubeconfig_names_as_the_other_choice() {
    let facts = Facts {
        engine: Engine::Ready,
        ..on_a_cluster(contexts(&["kind-team"], Some("kind-team")))
    };
    let setup = of(Kind::Docker, &facts, &Choices::default());
    let place = setup.step(StepId::Where).unwrap();
    assert_eq!(place.next.as_ref().unwrap().id, ActionId::UseDocker);
    assert_eq!(place.also.as_ref().unwrap().id, ActionId::UseCluster);
}

#[test]
fn the_cluster_is_recommended_where_docker_isnt_ready_and_a_context_is_found() {
    let found = contexts(&["kind-team"], Some("kind-team"));
    let none = Choices::default();
    assert_eq!(kind_of(&none, Engine::Missing, None, &found), Kind::Cluster);
    assert_eq!(kind_of(&none, Engine::Ready, None, &found), Kind::Docker);
    assert_eq!(
        kind_of(&none, Engine::Missing, None, &contexts(&[], None)),
        Kind::Docker
    );
    // What the person chose wins over what purlis recommends.
    let docker = Choices {
        kind: Some(Kind::Docker),
        ..Choices::default()
    };
    assert_eq!(
        kind_of(&docker, Engine::Missing, None, &found),
        Kind::Docker
    );
}

#[test]
fn a_cluster_with_nothing_on_it_passes_the_check_and_leaves_upstream_to_the_install() {
    let facts = on_a_cluster(chosen(nothing_installed(), empty_cluster()));
    let setup = of(Kind::Cluster, &facts, &chose("kind-team"));
    assert_eq!(
        states(&setup)[..4],
        [Done, Done, Current, Later],
        "{setup:#?}"
    );
    let check = setup.step(StepId::Check).unwrap();
    assert_eq!(check.line.as_deref(), Some("Ready for the install"));
    let said: Vec<(Verdict, &str, Option<&str>)> = check
        .checks
        .iter()
        .map(|line| (line.verdict, line.title.as_str(), line.more.as_deref()))
        .collect();
    assert_eq!(
        said,
        [
            (
                Verdict::Ok,
                "asked context kind-team, API server https://127.0.0.1:6443/",
                None
            ),
            (Verdict::Ok, "Kubernetes v1.36.4", None),
            (
                Verdict::Ok,
                "you may apply the install's cluster-scoped definitions",
                None
            ),
            (
                Verdict::Todo,
                "upstream agent-sandbox v1.0.6: not installed",
                Some("The Install step adds it.")
            ),
        ]
    );
    assert_eq!(setup.at, Some(StepId::Install));
}

#[test]
fn a_cluster_that_lacks_what_the_install_doesnt_add_stops_at_the_check_saying_what() {
    let mut report = nothing_installed();
    report.holds.remove(1);
    report.findings.push(doctor::Finding::new(
        "Kubernetes 1.36 or later: this cluster runs v1.33.2",
        "upgrade the cluster to Kubernetes 1.36 or later",
    ));
    let facts = on_a_cluster(chosen(report, empty_cluster()));
    let setup = of(Kind::Cluster, &facts, &chose("kind-team"));
    assert_eq!(setup.at, Some(StepId::Check));
    let check = setup.step(StepId::Check).unwrap();
    assert_eq!(check.state, Attention);
    assert!(
        check
            .checks
            .iter()
            .any(|line| line.verdict == Verdict::Missing
                && line.title == "Kubernetes 1.36 or later: this cluster runs v1.33.2"
                && line.more.as_deref()
                    == Some("Next: upgrade the cluster to Kubernetes 1.36 or later.")),
        "{check:#?}"
    );
    assert_eq!(check.next.as_ref().unwrap().id, ActionId::CheckAgain);
    // Nothing after it is offered.
    assert_eq!(setup.step(StepId::Install).unwrap().state, Later);
}

#[test]
fn an_installed_operator_shows_check_and_install_done_with_what_was_found() {
    let facts = on_a_cluster(ClusterFacts {
        chosen: Some("kind-team".into()),
        doctor: None,
        installed: Some(Ok(Installed {
            definitions: true,
            key: true,
            operator: Operator::Running {
                image: "ghcr.io/purlis/purlis-cluster-operator@sha256:abc".into(),
            },
            claims: Some((3, 2)),
        })),
        ..contexts(&["kind-team"], Some("kind-team"))
    });
    let setup = of(Kind::Cluster, &facts, &chose("kind-team"));
    assert_eq!(states(&setup)[..4], [Done, Done, Done, Current]);
    let check = setup.step(StepId::Check).unwrap();
    assert!(
        check.findings[0].contains("ghcr.io/purlis/purlis-cluster-operator@sha256:abc"),
        "{check:#?}"
    );
    assert_eq!(check.checks[0].title, "purlis's cluster operator runs");
    assert_eq!(setup.at, Some(StepId::Pool));
}

#[test]
fn a_context_chosen_is_kept_by_its_name_alone() {
    let config = tempfile::tempdir().unwrap();
    choose_cluster(config.path(), "kind-team").unwrap();
    let choices = read_choices(config.path()).unwrap();
    assert_eq!(choices.kind, Some(Kind::Cluster));
    assert_eq!(choices.context.as_deref(), Some("kind-team"));
    let kept =
        std::fs::read_to_string(crate::machine::dir(config.path()).join(CHOICES_FILE)).unwrap();
    let kept: serde_json::Value = serde_json::from_str(&kept).unwrap();
    let mut keys: Vec<&String> = kept.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["context", "kind", "version"], "{kept}");
}

#[test]
fn a_name_no_context_could_have_is_refused() {
    let config = tempfile::tempdir().unwrap();
    assert!(choose_cluster(config.path(), "--kubeconfig=/tmp/x").is_err());
    assert!(choose_cluster(config.path(), "two words\nhere").is_err());
}

/// A stand-in kubectl answering as a kind cluster with nothing installed answers, and recording
/// each run's words.
#[cfg(unix)]
fn fresh_kind(dir: &std::path::Path) -> Kubectl {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("kubectl");
    let ran = dir.join("ran");
    std::fs::write(
        &script,
        format!(
            r#"#!/bin/sh
printf '%s\n' "$*" >> '{ran}'
case "$*" in
  *"config view --minify --output json") printf '%s' '{{"clusters":[{{"cluster":{{"server":"https://127.0.0.1:6443/"}}}}]}}' ;;
  *"config view --output json") printf '%s' '{{"contexts":[{{"name":"kind-team"}}],"current-context":"kind-team"}}' ;;
  *"version --output json") printf '%s' '{{"serverVersion":{{"major":"1","minor":"36","gitVersion":"v1.36.4"}}}}' ;;
  *"auth can-i"*) echo yes ;;
  *"get namespaces --selector"*) printf '%s' '{{"items":[]}}' ;;
  *"--ignore-not-found"*) ;;
  *"get claims"*) printf '%s' '{{"items":[]}}' ;;
  *) echo "unexpected: $*" >&2; exit 3 ;;
esac
"#,
            ran = ran.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    Kubectl::at(script)
}

#[cfg(unix)]
#[test]
fn doctor_through_kubectl_says_of_a_fresh_cluster_what_the_operators_doctor_says() {
    let dir = tempfile::tempdir().unwrap();
    let kubectl = fresh_kind(dir.path());
    let facts = super::read(Ok(&kubectl), Some("kind-team"), true);
    let report = facts.doctor.clone().expect("doctor was asked");
    assert_eq!(
        report.to_string(),
        "ok: asked context kind-team, API server https://127.0.0.1:6443/\n\
         ok: Kubernetes v1.36.4\n\
         ok: you may apply the install's cluster-scoped definitions\n\
         missing: upstream agent-sandbox v1.0.6: not installed. Next: apply it, step 1 of the \
         install: kubectl apply --server-side -k operator/vendor/agent-sandbox/v1.0.6.\n\
         1 missing: install the cluster operator once doctor finds nothing missing"
    );
    assert_eq!(facts.installed, Some(Ok(empty_cluster())));
    // Every run was on the chosen context.
    let ran = std::fs::read_to_string(dir.path().join("ran")).unwrap();
    for line in ran
        .lines()
        .filter(|line| !line.starts_with("config view --output"))
    {
        assert!(line.starts_with("--context kind-team "), "{line}");
    }
}

#[cfg(unix)]
#[test]
fn the_signing_keys_secret_is_only_looked_for_never_read() {
    let dir = tempfile::tempdir().unwrap();
    let kubectl = fresh_kind(dir.path());
    let _ = super::read(Ok(&kubectl), Some("kind-team"), true);
    let ran = std::fs::read_to_string(dir.path().join("ran")).unwrap();
    let secrets: Vec<&str> = ran.lines().filter(|line| line.contains("secret")).collect();
    assert_eq!(secrets.len(), 1, "{ran}");
    // kubectl's table, which the API server fills with columns and never with what it holds.
    assert!(!secrets[0].contains("--output"), "{}", secrets[0]);
}

#[cfg(unix)]
#[test]
fn a_setup_on_docker_asks_the_cluster_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let kubectl = fresh_kind(dir.path());
    let facts = super::read(Ok(&kubectl), Some("kind-team"), false);
    assert_eq!(facts.contexts, ["kind-team"]);
    let ran = std::fs::read_to_string(dir.path().join("ran")).unwrap();
    assert_eq!(
        ran.lines().collect::<Vec<_>>(),
        ["config view --output json"]
    );
}

fn operator_runs(pools: &[&str]) -> ClusterFacts {
    ClusterFacts {
        chosen: Some("kind-team".into()),
        installed: Some(Ok(Installed {
            definitions: true,
            key: true,
            operator: Operator::Running {
                image: "ghcr.io/purlis/purlis-cluster-operator@sha256:abc".into(),
            },
            claims: Some((0, 0)),
        })),
        pools: Some(pools.iter().map(|pool| (*pool).to_owned()).collect()),
        ..contexts(&["kind-team"], Some("kind-team"))
    }
}

fn a_cluster_pool(owned: bool, mints: bool, sign_ins: SignIns) -> PoolFacts {
    PoolFacts {
        name: "team".into(),
        runner: Runner::Running,
        sign_ins,
        cluster: Some(KeptHere {
            context: "kind-team".into(),
            pool: "team".into(),
            owned,
            mints,
        }),
    }
}

#[test]
fn a_cluster_with_no_pool_offers_to_create_one() {
    let facts = on_a_cluster(operator_runs(&[]));
    let setup = of(Kind::Cluster, &facts, &chose("kind-team"));
    assert_eq!(setup.at, Some(StepId::Pool));
    let pool = setup.step(StepId::Pool).unwrap();
    assert_eq!(pool.next.as_ref().unwrap().id, ActionId::CreateClusterPool);
    let see = pool.see.as_ref().unwrap();
    assert!(see.text.contains("kind: Pool"), "{see:?}");
    assert!(
        see.text
            .starts_with("kubectl --context kind-team apply --server-side"),
        "{see:?}"
    );
}

#[test]
fn a_teammate_on_a_cluster_with_pools_is_offered_to_add_one_and_passes_install() {
    let facts = on_a_cluster(operator_runs(&["team", "gpu"]));
    let setup = of(Kind::Cluster, &facts, &chose("kind-team"));
    assert_eq!(states(&setup)[..4], [Done, Done, Done, Current]);
    let pool = setup.step(StepId::Pool).unwrap();
    assert!(
        pool.findings[0].contains("these pools: team, gpu"),
        "{pool:?}"
    );
    assert_eq!(pool.next.as_ref().unwrap().id, ActionId::AddClusterPool);
}

#[test]
fn a_cluster_pool_not_marked_as_yours_asks_to_be_marked_before_any_sign_in() {
    let facts = Facts {
        pool: Some(a_cluster_pool(false, false, SignIns::NotYours)),
        ..on_a_cluster(operator_runs(&["team"]))
    };
    let setup = of(Kind::Cluster, &facts, &chose("kind-team"));
    assert_eq!(setup.step(StepId::Pool).unwrap().state, Done);
    let sign_ins = setup.step(StepId::Signins).unwrap();
    assert_eq!(sign_ins.state, Current);
    assert!(
        sign_ins.findings[0].contains("isn't marked as your own"),
        "{sign_ins:?}"
    );
    assert_eq!(sign_ins.next.as_ref().unwrap().id, ActionId::MarkAsOwn);
}

#[test]
fn a_cluster_pool_that_doesnt_mint_shows_the_opt_in_by_hand_and_never_a_vaults_key() {
    let facts = Facts {
        pool: Some(a_cluster_pool(true, false, SignIns::Read(Vec::new()))),
        ..on_a_cluster(operator_runs(&["team"]))
    };
    let setup = of(Kind::Cluster, &facts, &chose("kind-team"));
    let see = setup.step(StepId::Identity).unwrap().see.clone().unwrap();
    assert!(see.text.contains("mintInCluster\":true"), "{}", see.text);
    assert!(
        see.text.contains("<the key GitHub gave you>.pem"),
        "{}",
        see.text
    );
    assert!(!see.text.contains("purlis secret"), "{}", see.text);
    assert!(!see.text.contains("vault "), "{}", see.text);
    // One that mints already shows no opt-in.
    let minting = Facts {
        pool: Some(a_cluster_pool(true, true, SignIns::Read(Vec::new()))),
        ..facts
    };
    let setup = of(Kind::Cluster, &minting, &chose("kind-team"));
    assert!(setup.step(StepId::Identity).unwrap().see.is_none());
}

#[test]
fn a_cluster_pools_setup_is_on_the_cluster_kind_whatever_was_chosen() {
    let docker = Choices {
        kind: Some(Kind::Docker),
        ..Choices::default()
    };
    assert_eq!(
        kind_of(&docker, Engine::Ready, Some(true), &ClusterFacts::default()),
        Kind::Cluster
    );
    assert_eq!(
        kind_of(
            &chose("x"),
            Engine::Ready,
            Some(false),
            &ClusterFacts::default()
        ),
        Kind::Docker
    );
}

#[cfg(unix)]
#[test]
fn the_pool_made_names_its_image_by_digest_and_nothing_else() {
    let image = "ghcr.io/purlis/purlis-runner@sha256:".to_owned() + &"a".repeat(64);
    assert_eq!(
        pool_definition("team", &image),
        format!(
            "apiVersion: runners.purlis.dev/v1alpha1\nkind: Pool\nmetadata:\n  name: team\nspec:\n  image: {image}\n"
        )
    );
    let dir = tempfile::tempdir().unwrap();
    let kubectl = fresh_kind(dir.path());
    assert!(create_pool(&kubectl, "team", "ghcr.io/purlis/purlis-runner:latest").is_err());
    assert!(create_pool(&kubectl, "Team!", &image).is_err());
    // Nothing reached the cluster.
    assert!(!dir.path().join("ran").exists());
}

#[test]
fn a_new_setup_forgets_the_context_chosen_and_keeps_the_operators_image() {
    let config = tempfile::tempdir().unwrap();
    choose_cluster(config.path(), "kind-team").unwrap();
    let image = "ghcr.io/purlis/purlis-cluster-operator@sha256:".to_owned() + &"d".repeat(64);
    choose_operator_image(config.path(), Some(&image)).unwrap();
    start_over(config.path()).unwrap();
    let choices = read_choices(config.path()).unwrap();
    assert_eq!(choices.kind, None);
    assert_eq!(choices.context, None);
    assert_eq!(choices.operator_image.as_deref(), Some(image.as_str()));
}
