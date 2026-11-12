// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Pool setup's cluster path on a real cluster** (#4492, #4493, spec #4469): the state module
//! read against a kind cluster with nothing of purlis on it, through `kubectl` with the run's own
//! kubeconfig, as a person's own sign-in. Where is done on the context chosen, Check the cluster
//! is doctor's own report, and the set-up stands at Install, since only upstream agent-sandbox is
//! missing and the install adds it. Then Install's parts are pressed one after another, each
//! read back done, a signing key that is there is kept when its part runs again, and the
//! operator runs. Then the Pool step makes a Pool, of this build's runner image, and a Claim on
//! it becomes ready (#4494): what the window's Add the pool here claims, and pairs, after. Then
//! a teammate holding the guide's rights alone joins (#4495), and reads the key the admin sees.
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

    // Only the context's name, the image and the made key's public half were kept.
    let kept = std::fs::read_dir(config.path()).unwrap().count();
    assert!(kept <= 1, "{kept} things kept");

    // This setup made the signing key, so its own Pool's key is pinned without a paste (#4495).
    let named = kubectl
        .clone()
        .with_context(&context)
        .pool_pin("setup-team")
        .unwrap()
        .key;
    assert_eq!(
        poolsetup::confirm_key(config.path(), &context, &named, None).unwrap(),
        purlis_core::clusterpool::Confirmed::MadeHere
    );

    a_teammate_with_the_guides_rights_joins_and_reads_the_key_the_admin_sees(
        &kubectl,
        &context,
        "setup-team",
    );
}

/// **A teammate joins** (#4495), on the cluster the admin just installed: a sign-in holding the
/// guide's rights alone (`get` and `use` on one Pool, Claims in a namespace of their own), as
/// `docs/cluster-pool-setup.md` grants them. Their Pool setup passes Check and Install as their
/// team's, asks doctor nothing, shows no admin's commands, asks for the pool's name since they
/// can't list the pools, and reads the same key through their sign-in as the admin sees through
/// theirs: the key the admin gives them, which pasted as an admin's chat wraps it, reads the same.
fn a_teammate_with_the_guides_rights_joins_and_reads_the_key_the_admin_sees(
    admin: &Kubectl,
    context: &str,
    pool: &str,
) {
    let on = admin.clone().with_context(context);
    let ran = |args: &[&str], input: Option<&str>| {
        let answer = on.asked(args, input).unwrap();
        assert!(answer.ok, "kubectl {args:?}: {}", answer.said);
        answer.printed
    };
    let (namespace, account) = ("setup-teammate", "teammate");
    let name = format!("system:serviceaccount:{namespace}:{account}");
    ran(&["create", "namespace", namespace], None);
    ran(
        &[
            "create",
            "serviceaccount",
            account,
            "--namespace",
            namespace,
        ],
        None,
    );
    let rights = format!(
        "apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: purlis-pool-{pool}
rules:
  - apiGroups: [runners.purlis.dev]
    resources: [pools]
    resourceNames: [{pool}]
    verbs: [get, use]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: purlis-pool-{pool}-{namespace}
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: purlis-pool-{pool}
subjects:
  - apiGroup: rbac.authorization.k8s.io
    kind: User
    name: {name}
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: makes-claims
  namespace: {namespace}
rules:
  - apiGroups: [runners.purlis.dev]
    resources: [claims]
    verbs: [create, get, patch, delete]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: makes-claims
  namespace: {namespace}
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: Role
  name: makes-claims
subjects:
  - apiGroup: rbac.authorization.k8s.io
    kind: User
    name: {name}
"
    );
    ran(&["apply", "-f", "-"], Some(&rights));
    let token = ran(
        &[
            "create",
            "token",
            account,
            "--namespace",
            namespace,
            "--duration",
            "1h",
        ],
        None,
    );
    // The teammate's own kubeconfig: the cluster's server and authority, their token, their
    // namespace. Nothing of the admin's sign-in.
    let mut view: serde_json::Value = serde_json::from_str(&ran(
        &[
            "config",
            "view",
            "--minify",
            "--raw",
            "--flatten",
            "--output",
            "json",
        ],
        None,
    ))
    .unwrap();
    view["users"] = serde_json::json!([{"name": "teammate", "user": {"token": token.trim()}}]);
    view["contexts"] = serde_json::json!([{"name": "teammate", "context": {
        "cluster": view["clusters"][0]["name"], "user": "teammate", "namespace": namespace,
    }}]);
    view["current-context"] = "teammate".into();
    let theirs = tempfile::tempdir().unwrap();
    let kubeconfig = theirs.path().join("config");
    std::fs::write(&kubeconfig, view.to_string()).unwrap();
    let teammate = Kubectl::at(admin.program()).with_kubeconfig(&kubeconfig);

    let config = tempfile::tempdir().unwrap();
    poolsetup::choose_cluster(config.path(), "teammate").unwrap();
    let here = Here {
        config: config.path(),
        plane: None,
        docker: None,
        engine: Err("this test asks no Docker".into()),
        kubectl: Ok(&teammate),
    };
    let setup = poolsetup::read(&here, None).unwrap();
    assert!(setup.contexts.joining, "{setup:#?}");
    for id in [StepId::Where, StepId::Check, StepId::Install] {
        assert_eq!(setup.step(id).unwrap().state, StepState::Done, "{setup:#?}");
    }
    let install = setup.step(StepId::Install).unwrap();
    assert_eq!(install.line.as_deref(), Some("Installed by your team"));
    assert!(install.parts.is_empty() && install.undo.is_none() && install.see.is_none());
    assert!(
        setup.step(StepId::Check).unwrap().checks.is_empty(),
        "doctor was asked"
    );
    assert!(setup.contexts.pools.is_empty(), "{setup:#?}");
    assert_eq!(setup.at, Some(StepId::Pool), "{setup:#?}");
    assert_eq!(
        setup
            .step(StepId::Pool)
            .unwrap()
            .next
            .as_ref()
            .map(|next| next.id),
        Some(poolsetup::ActionId::AddClusterPool),
        "{setup:#?}"
    );

    let admins = ran(
        &[
            "get",
            "pools.runners.purlis.dev",
            pool,
            "--output",
            "jsonpath={.status.signingKey}",
        ],
        None,
    );
    let read = teammate
        .clone()
        .with_context("teammate")
        .pool_pin(pool)
        .unwrap();
    assert_eq!(read.key.spelled(), admins.trim());
    let hex = admins.trim().trim_start_matches("ed25519:").to_uppercase();
    let pasted = format!("{}\n{}", &hex[..32], &hex[32..]);
    assert_eq!(
        purlis_core::clusterpool::OperatorKey::from_pasted(&pasted).unwrap(),
        read.key
    );
    // The teammate's machine made no key: nothing is pinned without the admin's, pasted (#4495).
    assert!(poolsetup::confirm_key(config.path(), "teammate", &read.key, None).is_err());
    assert_eq!(
        poolsetup::confirm_key(config.path(), "teammate", &read.key, Some(&pasted)).unwrap(),
        purlis_core::clusterpool::Confirmed::Pasted
    );
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
