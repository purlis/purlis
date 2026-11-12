// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Pool setup's cluster path on a real cluster** (#4492, #4493, spec #4469): the state module
//! read against a kind cluster with nothing of purlis on it, through `kubectl` with the run's own
//! kubeconfig, as a person's own sign-in. Where is done on the context chosen, Check the cluster
//! is doctor's own report, and the set-up stands at Install, since only upstream agent-sandbox is
//! missing and the install adds it. Then Install's parts are pressed one after another, each
//! read back done, a signing key that is there is kept when its part runs again, and the
//! operator runs. Then the Pool step makes a Pool, of this build's runner image, and a Claim on
//! it becomes ready (#4494): what the window's Add the pool here claims, and pairs, after.
//!
//! It needs a bare cluster's context in `POOL_SETUP_TEST_CONTEXT`, the operator's image by its
//! digest in `POOL_SETUP_TEST_OPERATOR_IMAGE`, the runner image by its digest in
//! `POOL_SETUP_TEST_RUNNER_IMAGE`, its kubeconfig in `KUBECONFIG` and `kubectl` on
//! `PATH`: `operator/e2e/kind.sh` makes one, apart from the cluster
//! its other tests install by the chart, and runs this with `--ignored`. It is ignored anywhere
//! else, and fails when any is missing.

#![cfg(unix)]

use std::time::{Duration, Instant};

use purlis_core::clusterpool::{Keeping, Kubectl};
use purlis_core::poolsetup::install::PartId;
use purlis_core::poolsetup::{self, CheckOf, Here, Kind, Setup, StepId, StepState, Verdict};

fn needed(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} names nothing: run operator/e2e/kind.sh"))
}

/// kubectl on `PATH`, on the run's own kubeconfig.
fn kubectl() -> Kubectl {
    let path = std::env::var("PATH").unwrap_or_default();
    let program = std::env::split_paths(&path)
        .map(|folder| folder.join("kubectl"))
        .find(|candidate| candidate.is_file())
        .expect("kubectl on PATH");
    Kubectl::at(program).with_kubeconfig(needed("KUBECONFIG"))
}

#[test]
#[ignore = "needs a bare kind cluster: operator/e2e/kind.sh runs it"]
fn a_bare_cluster_passes_the_check_then_is_installed_a_part_at_a_time() {
    purlis_core::unsteered!();
    let context = needed("POOL_SETUP_TEST_CONTEXT");
    let config = tempfile::tempdir().unwrap();
    poolsetup::choose_cluster(config.path(), &context).unwrap();
    let kubectl = kubectl();
    let here = Here {
        config: config.path(),
        plane: None,
        docker: None,
        engine: Err("this test asks no Docker".into()),
        kubectl: Ok(&kubectl),
    };
    let setup = poolsetup::read(&here, None).unwrap();
    assert_eq!(setup.kind, Kind::Cluster, "{setup:#?}");
    let state = |id| setup.step(id).unwrap().state;
    assert_eq!(state(StepId::Where), StepState::Done, "{setup:#?}");
    assert_eq!(state(StepId::Check), StepState::Done, "{setup:#?}");
    assert_eq!(setup.at, Some(StepId::Install), "{setup:#?}");
    let check = setup.step(StepId::Check).unwrap();
    assert_eq!(check.line.as_deref(), Some("Ready for the install"));
    let lines: Vec<(Verdict, &str)> = check
        .checks
        .iter()
        .filter(|line| line.about == CheckOf::Cluster)
        .map(|line| (line.verdict, line.title.as_str()))
        .collect();
    assert!(
        lines.contains(&(
            Verdict::Todo,
            "upstream agent-sandbox v1.0.6: not installed"
        )),
        "{lines:?}"
    );
    assert!(
        lines.contains(&(Verdict::Ok, "Kubernetes v1.36.4")),
        "{lines:?}"
    );
    assert!(
        lines.contains(&(
            Verdict::Ok,
            "you may apply the install's cluster-scoped definitions"
        )),
        "{lines:?}"
    );

    // Install, a part at a time, each read back done before the next is offered.
    poolsetup::choose_operator_image(
        config.path(),
        Some(&needed("POOL_SETUP_TEST_OPERATOR_IMAGE")),
    )
    .unwrap();
    let part_done = |setup: &Setup, id: PartId| {
        setup
            .step(StepId::Install)
            .unwrap()
            .parts
            .iter()
            .find(|part| part.id == id)
            .is_some_and(|part| part.done)
    };
    let mut settles_at = None;
    for id in PartId::ALL {
        let before = poolsetup::read(&here, None).unwrap();
        assert!(
            !part_done(&before, id),
            "{id:?} was done before it ran: {before:#?}"
        );
        let offered = before
            .step(StepId::Install)
            .unwrap()
            .parts
            .iter()
            .find(|part| part.id == id);
        assert!(
            offered.unwrap().next.is_some(),
            "{id:?} has no button: {before:#?}"
        );
        poolsetup::install_part(config.path(), &kubectl, id)
            .unwrap_or_else(|why| panic!("{id:?}: {why}"));
        if id == PartId::Definitions {
            // The operator acts on no Claim made within 30 seconds of its policy's binding.
            settles_at = Some(Instant::now() + Duration::from_secs(35));
        }
        let after = poolsetup::read(&here, None).unwrap();
        assert!(
            part_done(&after, id),
            "{id:?} wasn't read done after it ran: {after:#?}"
        );
    }
    // A key that is there is kept: the part runs again, and the Secret is the one made above.
    let made = secret_uid(&kubectl, &context);
    poolsetup::install_part(config.path(), &kubectl, PartId::SigningKey).unwrap();
    assert_eq!(
        secret_uid(&kubectl, &context),
        made,
        "the signing key was replaced"
    );

    let installed = poolsetup::read(&here, None).unwrap();
    assert_eq!(
        installed.step(StepId::Install).unwrap().state,
        StepState::Done,
        "{installed:#?}"
    );
    assert_eq!(
        installed.step(StepId::Check).unwrap().line.as_deref(),
        Some("purlis's cluster operator runs here")
    );
    assert_eq!(installed.at, Some(StepId::Pool), "{installed:#?}");

    // The Pool step: the cluster has no Pool, so it offers to make one, and then one to add.
    let pool_step = installed.step(StepId::Pool).unwrap();
    assert_eq!(
        pool_step.next.as_ref().map(|next| next.id),
        Some(poolsetup::ActionId::CreateClusterPool),
        "{installed:#?}"
    );
    poolsetup::create_cluster_pool(
        config.path(),
        &kubectl,
        "setup-team",
        &needed("POOL_SETUP_TEST_RUNNER_IMAGE"),
    )
    .unwrap();
    let with_a_pool = poolsetup::read(&here, None).unwrap();
    assert_eq!(
        with_a_pool.contexts.pools,
        ["setup-team"],
        "{with_a_pool:#?}"
    );
    assert_eq!(
        with_a_pool
            .step(StepId::Pool)
            .unwrap()
            .next
            .as_ref()
            .map(|next| next.id),
        Some(poolsetup::ActionId::AddClusterPool),
        "{with_a_pool:#?}"
    );

    // A Claim on it, as this kubeconfig's user, becomes ready.
    if let Some(at) = settles_at {
        std::thread::sleep(at.saturating_duration_since(Instant::now()));
    }
    let on = kubectl.clone().with_context(&context);
    let requester = on.whoami().unwrap();
    on.claim(
        "default",
        "setup-claim",
        "setup-team",
        &requester,
        Keeping::Fresh,
    )
    .unwrap();
    let waited = on
        .asked(
            &[
                "wait",
                "--for=condition=Ready",
                "claims.runners.purlis.dev/setup-claim",
                "--namespace",
                "default",
                "--timeout=300s",
            ],
            None,
        )
        .unwrap();
    assert!(
        waited.ok,
        "the Claim on Pool setup's pool never became ready: {} {:?}",
        waited.said,
        on.claim_said("default", "setup-claim")
    );

    // Only the context's name and the image were kept.
    let kept = std::fs::read_dir(config.path()).unwrap().count();
    assert!(kept <= 1, "{kept} things kept");
}

/// The signing key's Secret by its uid alone: what it holds is never asked.
fn secret_uid(kubectl: &Kubectl, context: &str) -> String {
    let answer = kubectl
        .clone()
        .with_context(context)
        .asked(
            &[
                "get",
                "secrets",
                "purlis-cluster-operator-signing-key",
                "--namespace",
                "purlis-system",
                "--output",
                "jsonpath={.metadata.uid}",
            ],
            None,
        )
        .unwrap();
    assert!(answer.ok, "{}", answer.said);
    answer.printed
}
