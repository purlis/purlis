// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::super::*;
use super::*;

use purlis_cluster_doctor as doctor;

/// A GitHub token's shape, as a cluster's error words could carry one along.
// Put together here, so no file of purlis's holds a token's shape whole.
const A_TOKEN: &str = concat!("gh", "p_", "R8fJ2kLmN4pQ6sT8vX0zB2dF4hJ6lN8pR0tV");
/// A bearer token, as an API server's words could echo a request's header.
const A_BEARER: &str = concat!(
    "Authorization: Bearer ",
    "eyJhbGciOiJSUzI1NiIsImtpZCI6IkJ4In0",
    ".",
    "eyJzdWIiOiJ0ZWFtbWF0ZSJ9.c2lnbmF0dXJl"
);

/// Doctor's report on a cluster where the person may not apply the install's definitions.
fn refused_the_rights() -> doctor::Report {
    doctor::Report {
        holds: vec![
            "asked context kind-team, API server https://127.0.0.1:6443/".into(),
            "Kubernetes v1.36.4".into(),
        ],
        findings: vec![doctor::Finding::new(
            "you may not apply the install's cluster-scoped definitions",
            "ask your cluster's admin to install purlis, or for the rights",
        )],
    }
}

fn on_the_cluster(report: doctor::Report) -> Facts {
    Facts {
        engine: Engine::Missing,
        connect: None,
        pool: None,
        identity: Identity::None,
        repos: true,
        cluster: cluster::ClusterFacts {
            contexts: vec!["kind-team".into()],
            current: Some("kind-team".into()),
            chosen: Some("kind-team".into()),
            doctor: Some(report),
            installed: Some(Ok(cluster::Installed {
                definitions: false,
                key: false,
                operator: cluster::Operator::Absent,
                claims: Some((0, 0)),
            })),
            ..cluster::ClusterFacts::default()
        },
    }
}

fn chose_the_cluster() -> Choices {
    Choices {
        kind: Some(Kind::Cluster),
        context: Some("kind-team".into()),
        ..Choices::default()
    }
}

#[test]
fn where_doctor_shows_the_person_cant_install_check_offers_the_words_for_their_admin() {
    let setup = of(
        Kind::Cluster,
        &on_the_cluster(refused_the_rights()),
        &chose_the_cluster(),
    );
    let check = setup.step(StepId::Check).unwrap();
    assert_eq!(check.state, StepState::Attention);
    assert_eq!(check.also.as_ref().unwrap().id, ActionId::SendToAdmin);
    let words = check.admin.as_deref().unwrap();
    for held in [
        "you may not apply the install's cluster-scoped definitions",
        GUIDE,
        "kubectl version",
        "kubectl apply --server-side -f operator/deploy/crds.yaml",
        "Let a teammate use the pool",
        env!("CARGO_PKG_VERSION"),
    ] {
        assert!(words.contains(held), "{held} isn't in:\n{words}");
    }
    // The admin runs them on their own context, not on the person's.
    assert!(!words.contains("--context"), "{words}");
}

#[test]
fn a_cluster_the_install_can_set_up_has_nothing_to_send() {
    let ready = doctor::Report {
        holds: vec!["Kubernetes v1.36.4".into()],
        findings: vec![doctor::Finding {
            install: true,
            ..doctor::Finding::new("upstream agent-sandbox v1.0.6: not installed", "apply it")
        }],
    };
    let setup = of(Kind::Cluster, &on_the_cluster(ready), &chose_the_cluster());
    let check = setup.step(StepId::Check).unwrap();
    assert_eq!(check.admin, None);
    assert_eq!(check.also, None);
}

/// **The words for the admin hold no secret value** (#4496's acceptance): a token or a bearer
/// header in what a cluster said is left out, every line that is left reads as no secret to the
/// scanner every guard of purlis's uses, and the rest of the words stand.
#[test]
fn the_words_for_the_admin_hold_no_secret_value_whatever_the_cluster_said() {
    let mut report = refused_the_rights();
    report.holds.push(format!("kubectl said: token {A_TOKEN}"));
    report.findings.push(doctor::Finding::new(
        format!("the API server answered: {A_BEARER}"),
        "check your sign-in",
    ));
    let setup = of(Kind::Cluster, &on_the_cluster(report), &chose_the_cluster());
    let words = setup.step(StepId::Check).unwrap().admin.clone().unwrap();
    assert!(!words.contains(A_TOKEN), "{words}");
    assert!(
        !words.contains("eyJhbGciOiJSUzI1NiIsImtpZCI6IkJ4In0"),
        "{words}"
    );
    assert!(words.contains(LEFT_OUT), "{words}");
    assert!(crate::secretshape::found(&words).is_none(), "{words}");
    for line in words.lines() {
        assert_eq!(crate::secretshape::secret_kind(line), None, "{line}");
        assert_eq!(crate::secretshape::token_kind(line), None, "{line}");
    }
    assert!(words.contains("kubectl apply --server-side -f operator/deploy/crds.yaml"));
}

#[test]
fn waiting_for_the_admin_is_its_own_state_and_says_so_with_a_way_to_stop() {
    let mut choices = chose_the_cluster();
    choices.admin = Some(Asked {
        context: "kind-team".into(),
        since: 1_800_000_000,
        project: None,
        ready: None,
    });
    let setup = of(
        Kind::Cluster,
        &on_the_cluster(refused_the_rights()),
        &choices,
    );
    let check = setup.step(StepId::Check).unwrap();
    assert_eq!(check.state, StepState::Waiting);
    assert_eq!(check.line.as_deref(), Some("Waiting for your admin"));
    assert_eq!(check.also.as_ref().unwrap().id, ActionId::StopWaiting);
    assert_eq!(check.next.as_ref().unwrap().id, ActionId::CheckAgain);
    assert!(check.admin.is_some(), "the words stay, to send again");
    assert_eq!(setup.at, Some(StepId::Check));
    // Asked about another context, this one isn't waiting.
    choices.admin.as_mut().unwrap().context = "prod".into();
    let setup = of(
        Kind::Cluster,
        &on_the_cluster(refused_the_rights()),
        &choices,
    );
    assert_eq!(
        setup.step(StepId::Check).unwrap().state,
        StepState::Attention
    );
}

/// **Closing and returning keeps the state** (#4496's acceptance): the ask is kept in the
/// choices' file, read back as it was, and forgotten only when asked to.
#[test]
fn asking_the_admin_is_kept_across_a_close_and_forgotten_only_on_purpose() {
    let config = tempfile::tempdir().unwrap();
    assert!(
        asked(config.path(), None, 1).is_err(),
        "no cluster chosen yet"
    );
    choose_cluster(config.path(), "kind-team").unwrap();
    let project = std::path::Path::new("/home/ada/team");
    asked(config.path(), Some(project), 1_800_000_000).unwrap();
    let back = read_choices(config.path()).unwrap();
    assert_eq!(
        back.admin,
        Some(Asked {
            context: "kind-team".into(),
            since: 1_800_000_000,
            project: Some(project.to_path_buf()),
            ready: None,
        })
    );
    assert!(waiting_on(&back, Some("kind-team")));
    // Another choice leaves it.
    choose_operator_image(config.path(), None).unwrap();
    assert!(read_choices(config.path()).unwrap().admin.is_some());
    forget(config.path()).unwrap();
    assert_eq!(read_choices(config.path()).unwrap().admin, None);
}

/// A stand-in kubectl, answering as a cluster where the person may do everything (`yes`) or
/// nothing (`no`), with nothing of purlis installed.
#[cfg(unix)]
fn a_cluster_answering(dir: &std::path::Path, may: &str) -> crate::clusterpool::Kubectl {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("kubectl");
    let exit = if may == "yes" { 0 } else { 1 };
    std::fs::write(
        &script,
        format!(
            r#"#!/bin/sh
case "$*" in
  *"config view --minify --output json") printf '%s' '{{"clusters":[{{"cluster":{{"server":"https://127.0.0.1:6443/"}}}}]}}' ;;
  *"config view --output json") printf '%s' '{{"contexts":[{{"name":"kind-team"}}],"current-context":"kind-team"}}' ;;
  *"version --output json") printf '%s' '{{"serverVersion":{{"major":"1","minor":"36","gitVersion":"v1.36.4"}}}}' ;;
  *"auth can-i"*) echo {may}; exit {exit} ;;
  *"api-resources"*) ;;
  *"get namespaces --selector"*) printf '%s' '{{"items":[]}}' ;;
  *"--ignore-not-found"*) ;;
  *"get claims"*) printf '%s' '{{"items":[]}}' ;;
  *) exit 3 ;;
esac
"#
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    crate::clusterpool::Kubectl::at(script)
}

/// **Ready raises one entry** (#4496's acceptance): the background recheck marks the ask ready
/// once the admin has given what the install can't add, and on that call alone; a cluster that
/// still lacks it, or no ask, marks nothing.
#[cfg(unix)]
#[test]
fn the_recheck_marks_the_ask_ready_once_when_the_cluster_no_longer_stops_the_setup() {
    let (config, dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let refused = a_cluster_answering(dir.path(), "no");
    assert!(
        !recheck(config.path(), Ok(&refused), 10).unwrap(),
        "nothing asked yet"
    );
    choose_cluster(config.path(), "kind-team").unwrap();
    asked(config.path(), None, 1).unwrap();
    assert!(!recheck(config.path(), Ok(&refused), 10).unwrap());
    assert_eq!(ready(&read_choices(config.path()).unwrap()), None);

    let other = tempfile::tempdir().unwrap();
    let granted = a_cluster_answering(other.path(), "yes");
    assert!(recheck(config.path(), Ok(&granted), 20).unwrap());
    assert_eq!(
        ready(&read_choices(config.path()).unwrap()).and_then(|asked| asked.ready),
        Some(20)
    );
    assert!(
        !recheck(config.path(), Ok(&granted), 30).unwrap(),
        "ready is raised once"
    );
    assert!(!recheck(config.path(), Err("no kubectl".into()), 40).unwrap());
}

/// **Every secret shape a cluster's words can carry is left out, as written or encoded**: what the
/// person shares goes through the scanner a save's guard reads with, whole and a line at a time,
/// and through every escaped and encoded reading of each line. Each value is put together here,
/// so no file of purlis's holds one whole.
#[test]
fn a_secret_is_left_out_however_it_is_spelled_or_encoded() {
    let planted = [
        // A Google sign-in's access token, as a GKE API server's error could echo its header.
        concat!(
            "Authorization: Bearer ya",
            "29.a0AfB_byC3dE5fG7hJ9kL1mN3pQ5rS7tV9xZ1bD3fH5jL7nP9"
        ),
        // An EKS token, as `aws eks get-token` hands kubectl one.
        concat!(
            "token: k8s-aws-v",
            "1.aHR0cHM6Ly9zdHMudXMtZWFzdC0xLmFtYXpvbmF3cy5jb20vP0FjdGlvbj1HZXRDYWxsZXJJZGVudGl0eQ"
        ),
        // A kubeconfig's client key, a PEM private key in base64.
        concat!(
            "client-key-data: LS0tLS1CRUdJTiBFQyBQUklW",
            "QVRFIEtFWS0tLS0tCk1IY0NBUUVFSUJhc2U2NGJhc2U2NGJhc2U2NGJhc2U2NGJhc2U2NAotLS0tLUVORCBFQyBQUklWQVRFIEtFWS0tLS0tCg=="
        ),
        // A registry's address with its password in it.
        concat!(
            "pulling from https://ada:",
            "Hunter2-s3cret-Pw",
            "@registry.example.com/v2/"
        ),
        // A JWT inside JSON, its dots escaped as JSON may spell them.
        concat!(
            "{\"id_token\":\"eyJhbGciOiJSUzI1NiIsImtpZCI6IkJ4In0",
            "\\u002e",
            "eyJzdWIiOiJ0ZWFtbWF0ZSJ9\\u002ec2lnbmF0dXJlc2lnbmF0dXJl\"}"
        ),
        // A GitHub token, percent-encoded as a URL's query carries it.
        concat!(
            "?access_token=%67%68%70%5F",
            "R8fJ2kLmN4pQ6sT8vX0zB2dF4hJ6lN8pR0tV"
        ),
    ];
    let kept: Vec<&str> = planted
        .iter()
        .copied()
        .filter(|secret| !held_free_of_secrets(&format!("a\n{secret}\nb")).contains(LEFT_OUT))
        .collect();
    assert!(kept.is_empty(), "kept: {kept:#?}");
    for secret in planted {
        let text = format!("doctor said:\n{secret}\nthe rest stands");
        let held = held_free_of_secrets(&text);
        assert!(held.contains(LEFT_OUT), "{secret} was kept: {held}");
        assert!(held.contains("the rest stands"), "{held}");
        assert!(
            crate::secretshape::leak_spans_whole_and_by_line(&held).is_empty(),
            "{held}"
        );
        for line in held.lines() {
            assert_eq!(crate::secretshape::token_kind_as_read(line), None, "{line}");
        }
    }
}
