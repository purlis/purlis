// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis-cluster-operator`: purlis's cluster operator, run in the cluster by
//! `deploy/operator.yaml` with its service account's credential.
//!
//! `purlis-cluster-operator doctor` is run by a team admin before installing it: it says what
//! the cluster lacks, with the admin's own credentials, and changes nothing (#2236).

use std::process::ExitCode;

const USAGE: &str = "usage: purlis-cluster-operator [doctor]

  (no command)  run the cluster operator, in the cluster
  doctor        say what this cluster lacks for the cluster operator, before
                installing it; reads only, with your kubeconfig's current
                context, and exits 1 when anything is missing";

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        [] => operate().await,
        ["doctor"] => doctor().await,
        ["-h" | "--help" | "help"] => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

async fn operate() -> ExitCode {
    tracing_subscriber::fmt().with_target(false).init();
    let client = match kube::Client::try_default().await {
        Ok(client) => client,
        Err(error) => {
            tracing::error!(%error, "could not reach the cluster's API server");
            return ExitCode::FAILURE;
        }
    };
    // Mounted from its Secret by the kubelet: the operator reads no Secret itself (#2056).
    let path = purlis_cluster_operator::vouch::SIGNING_KEY_PATH;
    let signing = match std::fs::read_to_string(path)
        .map_err(|error| error.to_string())
        .and_then(|pem| purlis_cluster_operator::vouch::SigningKey::from_pem(&pem))
    {
        Ok(signing) => signing,
        Err(error) => {
            tracing::error!(%error, %path, "no signing key: the operator vouches for no runner without one (docs/cluster-operator.md)");
            return ExitCode::FAILURE;
        }
    };
    tracing::info!(signing_key = %signing.public(), "signing runners' link keys");
    // The chart's perPerson.quota (#2055): an operator that cannot read it does not start.
    let var = purlis_cluster_operator::person::QUOTA_VAR;
    let quota = match purlis_cluster_operator::person::Quota::from_var(
        std::env::var(var).ok().as_deref(),
    ) {
        Ok(quota) => quota,
        Err(error) => {
            tracing::error!(%error, "no per-person quota (docs/cluster-operator.md)");
            return ExitCode::FAILURE;
        }
    };
    purlis_cluster_operator::run(client, signing, quota).await;
    ExitCode::SUCCESS
}

async fn doctor() -> ExitCode {
    use purlis_cluster_operator::doctor;
    let cluster = match doctor::Kube::from_environment().await {
        Ok(cluster) => cluster,
        Err(error) => {
            println!(
                "{}",
                doctor::Finding::new(
                    format!("a kubeconfig doctor can use: {error}"),
                    "point KUBECONFIG at the cluster's, or pick its context with kubectl config use-context, then run doctor again",
                )
            );
            return ExitCode::FAILURE;
        }
    };
    let report = doctor::check(&cluster).await;
    println!("{report}");
    if report.lacks_anything() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
