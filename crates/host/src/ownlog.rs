// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A host's own event log, read over a link from a cursor of the reader's** (1.41, #3393, ADR
//! 0078 §7: *"the desktop subscribes with a cursor per device. The runner's host is the only
//! writer of the runner's event log. The desktop never copies the runner's events into its
//! own"*).
//!
//! `log_end` says where the log ends now, by its device, so a reader that holds none of it
//! starts from there. A subscription whose `since` names the host's device is then pushed each
//! line after that cursor, and each as it is written, looked for every [`LOOK_EVERY`]; a gap the
//! log's retention left is pushed as `missed`, never skipped in silence. Only a link that may
//! read the log is pushed it ([`purlis_session_protocol::grants::Power::EventLog`]): the window,
//! and its desktop's host relaying it. The reading is the binary's ([`crate::Record::read_log`]),
//! as the writing is, so this crate links no core.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_session_protocol::grants::Power;
use purlis_session_protocol::session::{self, Peer, Pushed, Since};
use tokio::sync::mpsc;

use crate::hosting::Hosting;

/// How often the log is looked at for what was written since, while a reader is subscribed.
pub const LOOK_EVERY: Duration = Duration::from_secs(1);

impl Hosting {
    /// **Pushes this host's own event log to `told`** from the cursor `since` holds for this
    /// host's device, where `peer` may read it and named the device; until `told` closes.
    pub(crate) fn push_its_log(&self, since: &Since, peer: &Peer, told: mpsc::Sender<Pushed>) {
        if !peer.scope().may(Power::EventLog) || peer.version() < session::KNOWS_EVENT_LOG {
            return;
        }
        let hosting = self.clone();
        let since = since.clone();
        tokio::spawn(async move {
            let asking = hosting.clone();
            let Ok(Ok((device, _))) =
                tokio::task::spawn_blocking(move || asking.record().log_end()).await
            else {
                return;
            };
            let Some(after) = since.get(&device).copied() else {
                return;
            };
            let opening = hosting.clone();
            let reader =
                match tokio::task::spawn_blocking(move || opening.record().read_log(after)).await {
                    Ok(Ok(reader)) => Arc::new(Mutex::new(reader)),
                    Ok(Err(why)) => {
                        crate::say(&format!(
                            "a link asked for this host's event log, which could not be read: {why}"
                        ));
                        return;
                    }
                    Err(_) => return,
                };
            loop {
                let reading = Arc::clone(&reader);
                let read = tokio::task::spawn_blocking(move || {
                    reading
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .read()
                })
                .await;
                match read {
                    Ok(Ok(lines)) => {
                        for line in lines {
                            if told.send(line).await.is_err() {
                                return;
                            }
                        }
                    }
                    Ok(Err(why)) => crate::say(&format!("the event log could not be read: {why}")),
                    Err(_) => return,
                }
                tokio::select! {
                    () = told.closed() => return,
                    () = tokio::time::sleep(LOOK_EVERY) => {}
                }
            }
        });
    }

    /// Where this host's own event log ends now, as `log_end` answers it.
    pub(crate) async fn its_log_end(&self) -> Result<(String, u64), String> {
        let asking = self.clone();
        tokio::task::spawn_blocking(move || asking.record().log_end())
            .await
            .map_err(|why| why.to_string())?
    }
}
