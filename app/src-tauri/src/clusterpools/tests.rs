// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[cfg(unix)]
const OPERATOR: &str = "ed25519:abababababababababababababababababababababababababababababababab";
#[cfg(unix)]
const CLUSTER: &str = "3e9a4b2c-7d10-4f5e-8a6b-9c0d1e2f3a4b";
#[cfg(unix)]
const OTHER: &str = "ed25519:cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";

/// What the Notice reads follows what `runners.json` keeps (#4442).
#[cfg(unix)]
#[test]
fn the_pools_told_of_are_those_that_began_minting_until_it_is_put_away() {
    let config = tempfile::tempdir().unwrap();
    a_cluster_pool_kept(config.path());
    assert!(minting_without_asking(config.path()).unwrap().is_empty());
    assert!(purlis_core::runners::keep_cluster_minting(config.path(), "team", true).unwrap());
    assert_eq!(minting_without_asking(config.path()).unwrap(), ["team"]);
    purlis_core::runners::put_away_mint_notice(config.path(), "team").unwrap();
    assert!(minting_without_asking(config.path()).unwrap().is_empty());
}

/// A `runners.json` holding the cluster pool `team` and its runner, under `config`.
#[cfg(unix)]
pub(crate) fn a_cluster_pool_kept(config: &Path) {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let folder = purlis_core::machine::dir(config);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
    let file = serde_json::json!({
        "version": 1,
        "pools": [{
            "name": "team", "kind": "cluster", "kubeconfig": "/home/ada/.kube/config",
            "context": "team", "pool": "default", "namespace": "ada", "operatorKey": OPERATOR,
            "cluster": CLUSTER, "holds": ["/opt/homebrew/bin/kubectl"],
        }],
        "runners": [{
            "name": "team", "pool": "cluster", "namespace": "ada", "claim": "team-1",
            "claimUid": "6f1c2b8e-0f43-4c2e-9d7a-1b2c3d4e5f60", "requester": "ada@example.com",
            "pod": "team-1", "device": "01JB0RUNNER0000000000000000", "key": "4d".repeat(32),
            "paired": 1_800_000_000_u64,
        }],
    });
    let mut written = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(folder.join(purlis_core::machine::RUNNERS_FILE))
        .unwrap();
    std::io::Write::write_all(&mut written, file.to_string().as_bytes()).unwrap();
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
    // A change of Runs on rechecks the marks other tests read (#4591).
    let _marks = crate::notheld::rechecking();
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

#[test]
fn a_context_or_pool_purlis_cannot_name_is_refused_before_kubectl_runs() {
    assert_eq!(usable("team", "default"), Ok(()));
    assert_eq!(
        usable("arn:aws:eks:eu-west-1:1:cluster/team", "default"),
        Ok(())
    );
    assert!(usable("--server=https://elsewhere", "default").is_err());
    assert!(usable("team", "-default").is_err());
    assert!(usable("team", "Default Pool").is_err());
}

/// **An add out of its wait offers double it, on the pool being added** (spec #4275).
#[test]
fn an_add_that_ran_out_says_its_limit_and_offers_double_on_that_pool() {
    use purlis_core::poollimits::{LimitId, attempt};
    let (said, refused) = attempt(|| {
        ran_out_vouching(
            "ran-out-of-vouching",
            std::time::Duration::from_secs(300),
            "the cluster operator did not vouch.",
        )
    });
    assert!(said.contains("raise it to 10 minutes"), "{said}");
    let refused = refused.expect("the wait it ran out of");
    assert_eq!(refused.pool, "ran-out-of-vouching");
    let hits = refused.hits;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].limit, LimitId::VouchWait);
    assert_eq!(hits[0].suggested, Some(10));
}

/// **Only an add the wait itself ended offers to raise it** (#4398): a definite refusal says
/// itself, however long it took to come.
#[test]
fn a_definite_refusal_after_the_whole_wait_offers_nothing() {
    use purlis_core::poollimits::attempt;
    use purlis_core::runners::Refused;
    let within = std::time::Duration::ZERO;
    let (refused, hit) = attempt(|| {
        add_refused(
            "refused-definitely",
            within,
            &Refused::Other("the cluster's policy refuses this claim".to_owned()),
        )
    });
    assert_eq!(hit, None, "{refused}");
    assert!(!refused.contains("raise it"), "{refused}");
    let (_, hit) = attempt(|| {
        add_refused(
            "ran-out-by-its-outcome",
            std::time::Duration::from_secs(300),
            &Refused::WaitRanOut("the cluster operator did not vouch.".to_owned()),
        )
    });
    assert_eq!(hit.expect("the wait it ran out of").hits.len(), 1);
}

/// **Only an operator's word gives a task a runner of its own** (#4569): an older operator ends
/// such a runner's pod while its desktop is away, and not knowing is the same no.
#[test]
fn a_task_stays_on_the_pools_runner_unless_its_operator_says_otherwise() {
    assert_eq!(no_fresh_runner_said("team", Ok(())), None);
    let older = no_fresh_runner_said(
        "team",
        Err("its cluster operator gives no word for what it does".to_owned()),
    )
    .expect("it says why");
    assert!(older.contains("pool team's own runner"), "{older}");
    assert!(older.contains("gives no word"), "{older}");
    assert!(older.contains("Update the operator"), "{older}");
}

/// **A project whose folder is no git repository gets no runner of its own** (D-4569-21): what a
/// task wrote outside its clones could never be proved home. Its tab says why.
#[test]
fn a_project_that_is_no_repository_keeps_its_task_on_the_pools_runner() {
    let dir = tempfile::tempdir().unwrap();
    let said = no_fresh_runner_for_the_project(dir.path(), "team").expect("it says why");
    assert!(said.contains("pool team's own runner"), "{said}");
    assert!(said.contains("not a git repository"), "{said}");

    std::fs::create_dir(dir.path().join(".git")).unwrap();
    assert_eq!(no_fresh_runner_for_the_project(dir.path(), "team"), None);
}

/// **The operator's word is read again at release** (#4569, D-4569-18): a word that lapsed since
/// the start keeps a runner whose work looks home, and says why.
#[test]
fn a_runner_whose_operator_no_longer_gives_its_word_is_kept_though_its_work_looks_home() {
    assert_eq!(kept_without_the_word(Ok(())), None);
    let Some(FreshEnd::Kept { why, gone }) = kept_without_the_word(Err(
        "its cluster operator's word for what it does no longer holds".to_owned(),
    )) else {
        panic!("kept");
    };
    assert!(!gone);
    assert!(why[0].contains("no longer holds"), "{why:?}");
}

/// **A release that failed says its work is all here only where a proof said so** (#5393): a
/// runner the person discarded may hold what is not.
#[test]
fn a_release_that_failed_says_its_work_is_here_only_on_proof() {
    assert!(release_failed_said(true, "no route").starts_with("Its work is all here"));
    let discarded = release_failed_said(false, "no route");
    assert!(!discarded.contains("all here"), "{discarded}");
    assert!(discarded.contains("no route"), "{discarded}");
}

/// **Adding a cluster pool pins only the key the person checked** (#4495), shown or pasted from
/// their cluster's admin: a pasted key that isn't the Pool's refuses the add before anything is
/// kept or claimed, and says to stop.
#[cfg(unix)]
#[test]
fn an_add_with_a_pasted_key_that_isnt_the_pools_is_refused_before_anything_is_kept() {
    let (config, dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let kubectl = kubectl_answering(
        dir.path(),
        &serde_json::json!({"status": {"signingKey": OPERATOR, "cluster": CLUSTER}}).to_string(),
    );
    let adding = |key: &str| ClusterPoolAdding {
        name: "team".into(),
        kubeconfig: Some("/home/ada/.kube/config".into()),
        context: "team".into(),
        pool: "team".into(),
        key: key.into(),
        vouch_wait: None,
    };
    let within = std::time::Duration::ZERO;
    let home = Some(Path::new("/home/ada"));
    let why = add_on(config.path(), &kubectl, &adding(OTHER), home, within).unwrap_err();
    assert!(why.contains("Stop here"), "{why}");
    let why = add_on(config.path(), &kubectl, &adding("not a key"), home, within).unwrap_err();
    assert!(why.contains("isn't a cluster operator's key"), "{why}");
    assert!(
        purlis_core::runners::cluster_pools(config.path())
            .unwrap()
            .is_empty()
    );
    // The Pool's own key, pasted as an admin's chat wraps it, passes the check: the add goes
    // on, and stops later at what this stand-in can't answer, never at the key.
    let pasted = OPERATOR.trim_start_matches("ed25519:").to_uppercase();
    let wrapped = format!("{}\n{}", &pasted[..32], &pasted[32..]);
    if let Err(why) = add_on(config.path(), &kubectl, &adding(&wrapped), home, within) {
        assert!(!why.contains("Stop here"), "{why}");
        assert!(!why.contains("isn't a cluster operator's key"), "{why}");
    }
}
