// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis-cluster-operator`: purlis's cluster operator, run in the cluster by
//! `deploy/operator.yaml` with its service account's credential.

#[tokio::main]
async fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt().with_target(false).init();
    let client = match kube::Client::try_default().await {
        Ok(client) => client,
        Err(error) => {
            tracing::error!(%error, "could not reach the cluster's API server");
            return std::process::ExitCode::FAILURE;
        }
    };
    purlis_cluster_operator::run(client).await;
    std::process::ExitCode::SUCCESS
}
