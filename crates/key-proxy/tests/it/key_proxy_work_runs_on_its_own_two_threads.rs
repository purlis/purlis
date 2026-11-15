// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-054: the key proxy runs on a two-thread runtime of its own inside `purlisd`, never on the
//! host's current-thread runtime. This test is that host runtime: it starts a listener from inside
//! it, as the host's call does, and asks which threads served the requests.

use std::collections::HashSet;

use purlis_key_proxy::{KeyProxy, THREAD_NAME, WORKER_THREADS};

use crate::{Told, exchange};

#[tokio::test(flavor = "current_thread")]
async fn key_proxy_work_runs_on_its_own_two_threads() {
    let proxy = KeyProxy::new().expect("its runtime starts");
    let told = Told::default();
    let started = proxy
        .start("run-1", None, told.heard())
        .expect("a listener starts");

    // A probe spawned through the host runtime: where the host's own work runs.
    let probe = tokio::spawn(async { std::thread::current().id() })
        .await
        .unwrap();

    for _ in 0..16 {
        exchange(
            started.port,
            b"POST /v1/messages HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 2\r\n\r\n{}",
        )
        .await;
    }

    let records = told.records.lock().unwrap().clone();
    assert_eq!(records.len(), 16, "each request was served: {records:?}");
    let threads: HashSet<_> = records.iter().map(|(_, _, id)| *id).collect();
    assert!(
        !threads.contains(&probe),
        "a request was served on the host runtime's thread"
    );
    assert!(
        threads.len() <= WORKER_THREADS,
        "served on {} threads, more than its {WORKER_THREADS}",
        threads.len()
    );
    for (_, name, _) in &records {
        assert_eq!(
            name.as_deref(),
            Some(THREAD_NAME),
            "a request was served off the key proxy's runtime"
        );
    }
}
