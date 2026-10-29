// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[cfg(unix)]
const OPERATOR: &str = "ed25519:abababababababababababababababababababababababababababababababab";
#[cfg(unix)]
const CLUSTER: &str = "3e9a4b2c-7d10-4f5e-8a6b-9c0d1e2f3a4b";
#[cfg(unix)]
const OTHER: &str = "ed25519:cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";

/// A `runners.json` holding the cluster pool `team` and its runner, under `config`.
#[cfg(unix)]
fn a_cluster_pool_kept(config: &Path) {
    let folder =
        purlis_same_user::PrivateDirectory::open(&purlis_core::machine::dir(config)).unwrap();
    let file = serde_json::json!({
        "version": 1,
        "pools": [{
            "name": "team", "kind": "cluster", "kubeconfig": "/home/ada/.kube/config",
            "context": "team", "pool": "default", "namespace": "ada", "operatorKey": OPERATOR,
            "cluster": CLUSTER,
        }],
        "runners": [{
            "name": "team", "pool": "cluster", "namespace": "ada", "claim": "team-1",
            "claimUid": "6f1c2b8e-0f43-4c2e-9d7a-1b2c3d4e5f60", "requester": "ada@example.com",
            "pod": "team-1", "device": "01JB0RUNNER0000000000000000", "key": "ef".repeat(32),
            "paired": 1_800_000_000_u64,
        }],
    });
    folder
        .replace_file(
            purlis_core::machine::RUNNERS_FILE,
            file.to_string().as_bytes(),
        )
        .unwrap();
}

/// A stand-in for kubectl answering every run with `answer`.
#[cfg(unix)]
fn kubectl_answering(dir: &Path, answer: &str) -> Kubectl {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("kubectl");
    std::fs::write(
        &script,
        format!("#!/bin/sh\ncat > /dev/null\nprintf '%s\\n' '{answer}'\n"),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    Kubectl::at(script)
}

#[test]
fn a_kubeconfig_is_named_by_its_absolute_path_and_kubectls_own_default_when_none_is() {
    let home = Path::new("/home/ada");
    assert_eq!(
        kubeconfig_of(None, Some(home)).unwrap(),
        Path::new("/home/ada/.kube/config")
    );
    assert_eq!(
        kubeconfig_of(Some("  "), Some(home)).unwrap(),
        Path::new("/home/ada/.kube/config")
    );
    assert_eq!(
        kubeconfig_of(Some("~/work/kubeconfig"), Some(home)).unwrap(),
        Path::new("/home/ada/work/kubeconfig")
    );
    assert_eq!(
        kubeconfig_of(Some("/etc/team.yaml"), Some(home)).unwrap(),
        Path::new("/etc/team.yaml")
    );
    assert!(kubeconfig_of(Some("team.yaml"), Some(home)).is_err());
    assert!(kubeconfig_of(None, None).is_err());
}

#[cfg(unix)]
#[test]
fn a_cluster_pool_whose_pool_names_another_operator_key_is_said_as_its_key_changed() {
    let (config, dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    a_cluster_pool_kept(config.path());
    let kubectl = kubectl_answering(
        dir.path(),
        &serde_json::json!({"status": {"signingKey": OTHER, "cluster": CLUSTER}}).to_string(),
    );
    let linked = link_through(config.path(), &kubectl, "team", || {
        panic!("a runner whose operator key changed is never linked to")
    });
    assert_eq!(linked.unwrap(), PoolLink::OperatorKeyChanged);
}

#[test]
fn a_changed_runner_or_operator_key_is_told_to_the_link_as_a_changed_key() {
    use purlis_core::clusterpool::Refused;
    for changed in [
        Refused::KeyChanged {
            claim: "ada/team-1".into(),
        },
        Refused::OperatorKeyChanged {
            pool: "default".into(),
        },
    ] {
        assert!(matches!(
            crate::runnerlinks::not_checked(changed),
            purlis_host::NotChecked::KeyChanged(_)
        ));
    }
    assert!(matches!(
        crate::runnerlinks::not_checked(Refused::NotVouched("not yet".into())),
        purlis_host::NotChecked::Refused(_)
    ));
}

#[cfg(unix)]
#[test]
fn a_workspace_can_run_its_chats_on_a_cluster_pool_and_the_setting_offers_it() {
    let config = tempfile::tempdir().unwrap();
    a_cluster_pool_kept(config.path());
    let plane = Path::new("/planes/a");
    purlis_core::machine::update(config.path(), |store| store.remember(plane, 1)).unwrap();
    let set =
        crate::dockerpools::set_chats_run_on_at(config.path(), plane, "web", Some("team")).unwrap();
    assert_eq!(set.pool.as_deref(), Some("team"));
    assert_eq!(set.pools, ["team".to_owned()]);
    assert_eq!(
        purlis_core::pools::runs_on(config.path(), plane, Some("web")).unwrap(),
        Some("team".to_owned())
    );
}

#[cfg(unix)]
#[test]
fn a_cluster_pools_runner_is_reached_by_its_pools_name_alone() {
    let config = tempfile::tempdir().unwrap();
    a_cluster_pool_kept(config.path());
    let (pool, kept) = crate::runnerlinks::cluster_reached(config.path(), "team")
        .unwrap()
        .unwrap();
    assert_eq!((pool.name(), kept.pod.as_str()), ("team", "team-1"));
    assert!(
        crate::runnerlinks::cluster_reached(config.path(), "team/team")
            .unwrap()
            .is_some()
    );
    assert!(crate::runnerlinks::cluster_reached(config.path(), "team/other").is_err());
    assert!(
        crate::runnerlinks::cluster_reached(config.path(), "builds")
            .unwrap()
            .is_none()
    );
}
