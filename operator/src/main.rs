// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis-cluster-operator`: purlis's cluster operator, run in the cluster by
//! `deploy/operator.yaml` with its service account's credential.
//!
//! `purlis-cluster-operator doctor` is run by a team admin before installing it: it says what
//! the cluster lacks, with the admin's own credentials, and changes nothing (#2236).

use std::process::ExitCode;

const USAGE: &str = "usage: purlis-cluster-operator [doctor]

  (no command)  run the operator, in the cluster
  doctor        say what this cluster lacks for the operator, before installing it;
                reads only, with your kubeconfig's current context, and exits 1
                when anything is missing";

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
    purlis_cluster_operator::run(client).await;
    ExitCode::SUCCESS
}

async fn doctor() -> ExitCode {
    use purlis_cluster_operator::doctor;
    let client = match kube::Client::try_default().await {
        Ok(client) => client,
        Err(error) => {
            println!(
                "{}",
                doctor::Finding {
                    lacks: format!("a kubeconfig doctor can use: {error}"),
                    next: "point KUBECONFIG at the cluster's, or pick its context with kubectl config use-context, then run doctor again".to_owned(),
                }
            );
            return ExitCode::FAILURE;
        }
    };
    let report = doctor::check(&doctor::Kube(client)).await;
    println!("{report}");
    if report.lacks_anything() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
