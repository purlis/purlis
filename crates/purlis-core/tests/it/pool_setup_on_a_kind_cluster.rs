// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Pool setup's cluster path on a real cluster** (#4492, spec #4469): the state module read
//! against a kind cluster with nothing of purlis on it, through `kubectl` with the run's own
//! kubeconfig, as a person's own sign-in. Where is done on the context chosen, Check the cluster
//! is doctor's own report, and the set-up stands at Install, since only upstream agent-sandbox is
//! missing and the install adds it.
//!
//! It needs a bare cluster's context in `POOL_SETUP_TEST_CONTEXT`, its kubeconfig in
//! `KUBECONFIG` and `kubectl` on `PATH`: `operator/e2e/kind.sh` makes one, apart from the cluster
//! its other tests install by the chart, and runs this with `--ignored`. It is ignored anywhere
//! else, and fails when any is missing.

#![cfg(unix)]

use purlis_core::clusterpool::Kubectl;
use purlis_core::poolsetup::{self, CheckOf, Here, Kind, StepId, StepState, Verdict};

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
fn a_bare_cluster_passes_the_check_and_stands_at_the_install() {
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
        lines.contains(&(Verdict::Todo, "upstream agent-sandbox v1.0.6: not installed")),
        "{lines:?}"
    );
    assert!(lines.contains(&(Verdict::Ok, "Kubernetes v1.36.4")), "{lines:?}");
    assert!(
        lines.contains(&(Verdict::Ok, "you may apply the install's cluster-scoped definitions")),
        "{lines:?}"
    );
    // Only the context's name was kept.
    let kept = std::fs::read_dir(config.path()).unwrap().count();
    assert!(kept <= 1, "{kept} things kept");
}
