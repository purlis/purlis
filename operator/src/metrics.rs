// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The operator's metrics** (#3737), in Prometheus's text format on its own port
//! ([`PORT`], served at `/metrics`), as the person ruled on 2026-10-29:
//!
//! - `purlis_cluster_operator_claims` (gauge: `pool`, `namespace`, `state`): the Claims the
//!   operator watches, `state` being `Ready` when their runner is ready and otherwise their
//!   `Ready` condition's reason (`Unknown` before the operator has said one);
//! - `purlis_cluster_operator_warm_runners` (gauge: `pool`, `namespace`): the runners upstream
//!   says are warm and ready in each runner namespace's warm pool of a pool;
//! - `purlis_cluster_operator_suspensions_total` (counter: `pool`, `cause`): the runners the
//!   operator suspended;
//! - `purlis_cluster_operator_claims_deleted_total` (counter: `pool`, `cause`): the Claims whose
//!   runner the operator removed as they were deleted.
//!
//! The gauges are read from what the operator's watches hold as they are scraped; the counters
//! count from the operator's start, as Prometheus counters do.

use crate::person::NAMESPACE_PREFIX;
use crate::resources::{Claim, READY};
use crate::upstream::{self, SandboxWarmPool};
use futures::StreamExt;
use kube::Api;
use kube::runtime::reflector::{self, Store, reflector};
use kube::runtime::{WatchStreamExt, watcher};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

/// The port the operator serves its metrics on, in its pod and on its metrics Service.
pub const PORT: u16 = 8080;

/// The path the metrics are served at.
pub const PATH: &str = "/metrics";

/// Why the operator suspended a runner or removed a Claim's runner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cause {
    /// The Claim or its Pool was set `stopped` (#2058).
    Stopped,
    /// Nothing ran on the runner for its pool's `idleAfter` (#3980).
    Idle,
    /// Someone deleted the Claim.
    Requested,
    /// Its retention ended (#3981).
    Retention,
}

impl Cause {
    fn label(self) -> &'static str {
        match self {
            Cause::Stopped => "stopped",
            Cause::Idle => "idle",
            Cause::Requested => "requested",
            Cause::Retention => "retention",
        }
    }
}

/// The operator's counters, shared by its control loop and its metrics server.
#[derive(Debug, Default)]
pub struct Metrics {
    suspensions: Mutex<BTreeMap<(String, Cause), u64>>,
    deletions: Mutex<BTreeMap<(String, Cause), u64>>,
}

fn count(counter: &Mutex<BTreeMap<(String, Cause), u64>>, pool: &str, cause: Cause) {
    let mut counts = counter
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let at = counts.entry((pool.to_owned(), cause)).or_insert(0);
    *at = at.saturating_add(1);
}

/// `value` as a label value in Prometheus's text format: `\`, `"` and line feeds escaped.
fn escaped(value: &str) -> String {
    value
        .replace('\\', r"\\")
        .replace('"', "\\\"")
        .replace('\n', r"\n")
}

/// A Claim's `state` label: `Ready` when its runner is ready, its `Ready` reason otherwise.
fn state_of(claim: &Claim) -> String {
    let ready = claim
        .status
        .as_ref()
        .and_then(|status| status.conditions.iter().find(|c| c.type_ == READY));
    match ready {
        Some(ready) if ready.status == "True" => READY.to_owned(),
        Some(ready) if !ready.reason.is_empty() => ready.reason.clone(),
        _ => "Unknown".to_owned(),
    }
}

impl Metrics {
    /// One more runner of `pool` suspended, for `cause`.
    pub fn suspended(&self, pool: &str, cause: Cause) {
        count(&self.suspensions, pool, cause);
    }

    /// One more Claim on `pool` whose runner was removed as it was deleted, for `cause`.
    pub fn deleted(&self, pool: &str, cause: Cause) {
        count(&self.deletions, pool, cause);
    }

    /// Every metric, in Prometheus's text format, from `claims` and `warm_pools` as the
    /// operator's watches hold them. Only a warm pool named for a pool in a runner namespace is
    /// counted.
    pub fn render(&self, claims: &[Arc<Claim>], warm_pools: &[Arc<SandboxWarmPool>]) -> String {
        let mut by_state = BTreeMap::<(String, String, String), u64>::new();
        for claim in claims {
            let key = (
                claim.spec.pool.clone(),
                claim.metadata.namespace.clone().unwrap_or_default(),
                state_of(claim),
            );
            *by_state.entry(key).or_insert(0) += 1;
        }
        let mut warm = BTreeMap::<(String, String), i64>::new();
        for held in warm_pools {
            let namespace = held.metadata.namespace.as_deref().unwrap_or_default();
            let pool = held.metadata.name.as_deref().and_then(upstream::pool_of);
            let Some(pool) = pool.filter(|_| namespace.starts_with(NAMESPACE_PREFIX)) else {
                continue;
            };
            let ready = held
                .status
                .as_ref()
                .and_then(|status| status.ready_replicas)
                .unwrap_or(0)
                .max(0);
            *warm
                .entry((pool.to_owned(), namespace.to_owned()))
                .or_insert(0) += i64::from(ready);
        }
        let mut out = String::new();
        let _ = writeln!(
            out,
            "# HELP purlis_cluster_operator_claims The Claims the operator watches, by pool, namespace and state.\n\
             # TYPE purlis_cluster_operator_claims gauge"
        );
        for ((pool, namespace, state), n) in &by_state {
            let _ = writeln!(
                out,
                "purlis_cluster_operator_claims{{pool=\"{}\",namespace=\"{}\",state=\"{}\"}} {n}",
                escaped(pool),
                escaped(namespace),
                escaped(state)
            );
        }
        let _ = writeln!(
            out,
            "# HELP purlis_cluster_operator_warm_runners The runners kept warm and ready, by pool and runner namespace.\n\
             # TYPE purlis_cluster_operator_warm_runners gauge"
        );
        for ((pool, namespace), n) in &warm {
            let _ = writeln!(
                out,
                "purlis_cluster_operator_warm_runners{{pool=\"{}\",namespace=\"{}\"}} {n}",
                escaped(pool),
                escaped(namespace)
            );
        }
        for (name, help, counter) in [
            (
                "suspensions_total",
                "The runners the operator suspended, by pool and cause.",
                &self.suspensions,
            ),
            (
                "claims_deleted_total",
                "The Claims whose runner the operator removed as they were deleted, by pool and cause.",
                &self.deletions,
            ),
        ] {
            let _ = writeln!(
                out,
                "# HELP purlis_cluster_operator_{name} {help}\n# TYPE purlis_cluster_operator_{name} counter"
            );
            let counts = counter
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            for ((pool, cause), n) in counts.iter() {
                let _ = writeln!(
                    out,
                    "purlis_cluster_operator_{name}{{pool=\"{}\",cause=\"{}\"}} {n}",
                    escaped(pool),
                    cause.label()
                );
            }
        }
        out
    }
}

/// The most of a request the metrics port reads: a scrape's request line and headers fit well
/// within it.
pub const REQUEST_LIMIT: usize = 8 * 1024;

/// How long the metrics port waits for a request, and for its answer to be taken.
const ANSWER_WITHIN: Duration = Duration::from_secs(2);

/// The most connections the metrics port holds open at once; one more is closed unanswered, so
/// no number of connections spends more than these of the operator's memory. A scraper or two
/// needs far fewer.
const CONNECTIONS: usize = 32;

/// How long the metrics port waits before accepting again after a connection could not be
/// accepted (no file descriptor left, say), so it never spins.
const ACCEPT_AGAIN_AFTER: Duration = Duration::from_millis(500);

/// The answer to a request whose head is `head` (its request line and headers): the metrics,
/// `rendered` only when it is `GET` [`PATH`]; 404 otherwise. Each connection is answered once
/// and closed.
pub fn answer(head: &str, rendered: impl FnOnce() -> String) -> String {
    let mut words = head.lines().next().unwrap_or_default().split(' ');
    let (method, target) = (words.next(), words.next());
    let path = target.map(|target| target.split('?').next().unwrap_or_default());
    let (status, kind, body) = if method == Some("GET") && path == Some(PATH) {
        (
            "200 OK",
            "text/plain; version=0.0.4; charset=utf-8",
            rendered(),
        )
    } else {
        ("404 Not Found", "text/plain; charset=utf-8", String::new())
    };
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: {kind}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// `stream`'s request head, read up to the blank line that ends it, [`REQUEST_LIMIT`] at most:
/// nothing past the limit is read.
pub async fn head_of(stream: &mut (impl AsyncRead + Unpin)) -> std::io::Result<String> {
    let mut read = Vec::new();
    let mut buffer = [0u8; 1024];
    while read.len() < REQUEST_LIMIT {
        let room = buffer.len().min(REQUEST_LIMIT - read.len());
        let n = stream.read(&mut buffer[..room]).await?;
        if n == 0 {
            break;
        }
        // Only where the new bytes can end the head is looked at, never the whole head again.
        let from = read.len().saturating_sub(3);
        read.extend_from_slice(&buffer[..n]);
        if read[from..].windows(4).any(|end| end == b"\r\n\r\n") {
            break;
        }
    }
    Ok(String::from_utf8_lossy(&read).into_owned())
}

/// **Serve the metrics on [`PORT`]** until the process ends: `GET` [`PATH`] answers
/// [`Metrics::render`] of `claims`, the Claims the control loop's watch holds, and of every
/// upstream warm pool, which this watches itself ([`answer`]). Plain HTTP/1.1 over tokio's own
/// listener, so the port adds no crate to the operator.
pub async fn serve(client: kube::Client, metrics: Arc<Metrics>, claims: Store<Claim>) {
    let (warm_pools, writer) = reflector::store();
    let watched = reflector(
        writer,
        watcher(Api::<SandboxWarmPool>::all(client), watcher::Config::default()),
    )
    .default_backoff()
    .touched_objects()
    .for_each(|event| async move {
        if let Err(error) = event {
            tracing::warn!(%error, "the warm pools could not be watched for the metrics; trying again");
        }
    });
    tokio::spawn(watched);
    // Every address, IPv6 and IPv4 alike where the pod has IPv6, IPv4's alone where it has not.
    let bound = match TcpListener::bind(("::", PORT)).await {
        Ok(listener) => Ok(listener),
        Err(_) => TcpListener::bind(("0.0.0.0", PORT)).await,
    };
    let listener = match bound {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!(%error, port = PORT, "the metrics could not be served");
            return;
        }
    };
    tracing::info!(port = PORT, path = PATH, "serving metrics");
    let open = Arc::new(Semaphore::new(CONNECTIONS));
    loop {
        let mut stream = match listener.accept().await {
            Ok((stream, _)) => stream,
            Err(error) => {
                tracing::warn!(%error, "a metrics connection was not accepted");
                tokio::time::sleep(ACCEPT_AGAIN_AFTER).await;
                continue;
            }
        };
        // Past the cap, the connection is dropped, which closes it.
        let Ok(permit) = open.clone().try_acquire_owned() else {
            continue;
        };
        let (metrics, claims, warm_pools) = (metrics.clone(), claims.clone(), warm_pools.clone());
        tokio::spawn(async move {
            let _permit = permit;
            let answered = tokio::time::timeout(ANSWER_WITHIN, async {
                let head = head_of(&mut stream).await?;
                let answer = answer(&head, || {
                    metrics.render(&claims.state(), &warm_pools.state())
                });
                stream.write_all(answer.as_bytes()).await?;
                stream.shutdown().await
            })
            .await;
            if let Ok(Err(error)) = answered {
                tracing::debug!(%error, "a metrics connection ended");
            }
        });
    }
}
