// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A run's key proxy in `purlis serve`, a runner's session host** (T1a-04, R-053): the runner's
//! binary's [`purlis_host::ProxiesKeys`], as the window's binary has its own, which owns the key
//! proxy's runtime and its crate, so the host links no HTTP code. Only the window's link starts
//! one, and a runner's link is never the window's, so none is started here yet: a runner's key
//! proxy starts from its desktop's plan in phase 1a's set iii. The runtime, two threads of its
//! own, is made at the first start and not before, so a host that never starts one has none
//! (R-062).

use std::sync::{Arc, OnceLock};

use purlis_host::{KeyProxyStarted, KeyProxyTell, KeyProxyTold, ProxiesKeys};
use purlis_key_proxy::{Heard, KeyProxy};

/// Starts and stops each run's key proxy on this machine.
#[derive(Default)]
pub struct ProxiesKeysHere(OnceLock<Result<KeyProxy, String>>);

impl ProxiesKeys for ProxiesKeysHere {
    fn start(&self, run: &str, tell: KeyProxyTell) -> Result<KeyProxyStarted, String> {
        let proxy = self
            .0
            .get_or_init(|| {
                KeyProxy::new().map_err(|why| format!("the key proxy could not start: {why}"))
            })
            .as_ref()?;
        let recorded = Arc::clone(&tell);
        let heard = Heard {
            say: Arc::new(move |line| tell(KeyProxyTold::Said(line.to_string()))),
            record: Arc::new(move |record| {
                recorded(KeyProxyTold::Record {
                    method: record.method.clone(),
                    path: record.path.clone(),
                    status: record.status,
                })
            }),
        };
        // No route yet: binding a run to its route is T1a-08's, so every request is refused.
        let started = proxy
            .start(run, None, heard)
            .map_err(|why| format!("the key proxy could not listen for this run: {why}"))?;
        Ok(KeyProxyStarted {
            port: started.port,
            token: started.token.reveal().to_string(),
        })
    }

    fn stop(&self, run: &str) -> Result<(), String> {
        match self.0.get() {
            Some(Ok(proxy)) if proxy.stop(run) => Ok(()),
            _ => Err("no key proxy runs for this run".into()),
        }
    }
}
