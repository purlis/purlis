// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A real Claude Code chat on a cluster pool's runner, held as on a Docker pool's** (#4083): the
//! probe [`super::a_real_claude_code_chat_on_a_runner_is_held_as_a_codex_chat_is`] runs on a Docker
//! pool's runner in the parity job, and here on the runner pod of a Pool of purlis's cluster
//! operator, at the floor and at the full sandbox. The pod is the cluster's: its security context
//! and seccomp profile are the operator's, not Docker's, and the wrap has to hold there all the
//! same. The program Claude Code ships runs with a stand-in model, and no credential is used.
//!
//! The pod starts no browser here, so the chat tries what needs none
//! ([`super::what_lies_past_a_chat_on`]).
//!
//! It needs a cluster with purlis's cluster operator installed. `operator/e2e/kind.sh` runs it with
//! `--ignored`, naming in `CLUSTER_POOL_TEST_PROBE_IMAGE` the parity job's image built on this
//! build's runner image (`tests/parity/runner.Dockerfile`), and in `CLUSTER_POOL_TEST_CONTEXT` the
//! kubeconfig's context. It is ignored anywhere else, and fails when either is missing.

use purlis_core::clusterpool::Kubectl;

use super::fresh_runner::{a_pool_on_the_cluster, kept_namespace};
use super::*;
use crate::runnerlinks::Machine;

/// The cluster pool `a_pool_on_the_cluster` adds, by the name the desktop keeps it under.
const POOL: &str = "team";

#[test]
#[ignore = "needs a cluster with purlis's cluster operator: operator/e2e/kind.sh"]
fn a_real_claude_code_chat_on_a_cluster_runner_is_held_as_on_a_docker_runner() {
    let image = std::env::var("CLUSTER_POOL_TEST_PROBE_IMAGE")
        .expect("CLUSTER_POOL_TEST_PROBE_IMAGE names the probe's runner image by digest");
    let context = std::env::var("CLUSTER_POOL_TEST_CONTEXT")
        .expect("CLUSTER_POOL_TEST_CONTEXT names the cluster's kubeconfig context");
    let config = tempfile::tempdir().expect("the desktop's config home");
    let config_path = config.path().canonicalize().expect("its folder");
    let machine = Machine {
        config: config_path.clone(),
        docker: purlis_core::dockerpool::Docker::at(std::ffi::OsString::from("docker")),
        keyring: Arc::new(purlis_core::secrets::keyring::FileStore::at(
            config_path.join("keyring.json"),
        )),
    };
    let kubectl = Kubectl::at("kubectl").with_context(&context);
    let (_removes, _namespace) = a_pool_on_the_cluster(&machine, &kubectl, &context, &image);

    // A window whose project's every chat starts on the pool's own runner.
    let WindowsHost {
        runtime,
        folder,
        data,
        a_connection,
        ..
    } = a_windows_host_for(&machine, crate::runnerlinks::relays_to(config_path.clone()));
    let mut target = a_window_over(
        a_connection(),
        Vec::new(),
        data.path().to_path_buf(),
        Runs::OnAPoolsRunner {
            pool: POOL.to_owned(),
            config: config_path.clone(),
        },
    );
    a_repository(&target.root);
    let (_, kept) = purlis_core::runners::cluster_runner_for(&config_path, POOL)
        .expect("the file reads")
        .expect("the pool's runner is kept");
    let namespace = kept_namespace(&kubectl, &config_path, &kept);
    target.runner = Some(OnTheRunner::in_a_pod(
        kubectl.clone(),
        &namespace,
        &kept.pod,
        POOL,
    ));

    a_real_claude_code_chat_on_a_runner_is_held(&target, ClaudeFolder::Made);

    drop(target);
    runtime.shutdown_timeout(Duration::from_secs(5));
    drop((folder, data, config));
}
