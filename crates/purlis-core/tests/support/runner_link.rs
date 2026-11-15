// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An in-memory link between a desktop end and a runner end** (R-701, T1a-50), for the runner
//! tests of set (iii): the key-proxy plan with its key (T1a-51), the pool key allowance's check
//! (T1a-53), and the six messages of T1a-74, the pushed changes to your keys among them.
//!
//! The bytes cross the session protocol's own link simulator (`netsim.rs` in
//! `purlis-session-protocol`'s tests, FD-4), unshaped: no delay, no loss. Each message is one
//! line of JSON. A message counts as **crossed** once the far end has read it whole, and the
//! link records each one with the end it came from, so a test can say a message crossed once,
//! or never. [`InMemoryLink::drop_link`] drops it as a link that went down: a send then fails
//! with [`LinkDown`], and what was in flight never crosses. [`InMemoryLink::relink`] joins the
//! two ends again on a new link, and the record carries on.
//!
//! The link is generic over the message: each ticket that adds one of set (iii)'s messages
//! carries its own type over it, so no message is defined here ahead of the ticket that owns it.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, WriteHalf};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[path = "../../../session-protocol/tests/common/netsim.rs"]
mod netsim;

/// One end of the link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    Desktop,
    Runner,
}

/// A message that crossed, with the end it was sent from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crossed<M> {
    pub from: End,
    pub message: M,
}

/// The link is down: dropped, and not linked again yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkDown;

/// What the link carries unshaped: no delay, no loss, and room for any one message.
const UNSHAPED: netsim::Shape = netsim::Shape {
    one_way: Duration::ZERO,
    bytes_per_second: 1e12,
    buffer: 1 << 20,
    loss_one_in: 0,
    packet: 1500,
    retransmit: Duration::ZERO,
};

/// One end as the test holds it while the link is up: where it writes, and what it was sent.
struct Side<M> {
    writes: WriteHalf<DuplexStream>,
    inbox: mpsc::UnboundedReceiver<M>,
}

/// The two ends joined, and the tasks that read each one's far side.
struct Joined<M> {
    desktop: Side<M>,
    runner: Side<M>,
    readers: [JoinHandle<()>; 2],
}

impl<M> Drop for Joined<M> {
    fn drop(&mut self) {
        for reader in &self.readers {
            reader.abort();
        }
    }
}

/// **A desktop end and a runner end joined in memory**, recording each message that crossed.
/// Made inside a tokio runtime, which its readers run on.
pub struct InMemoryLink<M> {
    joined: Option<Joined<M>>,
    record: Arc<Mutex<Vec<Crossed<M>>>>,
    links: u64,
}

impl<M> InMemoryLink<M>
where
    M: Serialize + DeserializeOwned + Clone + Send + 'static,
{
    /// The two ends, linked.
    pub fn up() -> Self {
        let mut link = InMemoryLink {
            joined: None,
            record: Arc::new(Mutex::new(Vec::new())),
            links: 0,
        };
        link.relink();
        link
    }

    /// Drops the link, as one that went down: what was in flight is lost, and sends fail.
    pub fn drop_link(&mut self) {
        self.joined = None;
    }

    /// Joins the two ends again on a new link, dropping the one up now, if any.
    pub fn relink(&mut self) {
        self.joined = None;
        self.links += 1;
        let (desktop, runner) = netsim::link(UNSHAPED, self.links);
        let (desktop_reads, desktop_writes) = tokio::io::split(desktop);
        let (runner_reads, runner_writes) = tokio::io::split(runner);
        let (to_desktop, desktop_inbox) = mpsc::unbounded_channel();
        let (to_runner, runner_inbox) = mpsc::unbounded_channel();
        self.joined = Some(Joined {
            desktop: Side {
                writes: desktop_writes,
                inbox: desktop_inbox,
            },
            runner: Side {
                writes: runner_writes,
                inbox: runner_inbox,
            },
            readers: [
                // What the runner reads came from the desktop, and the other way round.
                tokio::spawn(reader(
                    runner_reads,
                    End::Desktop,
                    to_runner,
                    self.record.clone(),
                )),
                tokio::spawn(reader(
                    desktop_reads,
                    End::Runner,
                    to_desktop,
                    self.record.clone(),
                )),
            ],
        });
    }

    /// Whether the link is up.
    pub fn is_up(&self) -> bool {
        self.joined.is_some()
    }

    /// Sends `message` from `end` to the other end, or fails with [`LinkDown`].
    pub async fn send(&mut self, end: End, message: M) -> Result<(), LinkDown> {
        let side = self.side(end).ok_or(LinkDown)?;
        let mut line = serde_json::to_vec(&message).expect("a message is JSON");
        line.push(b'\n');
        side.writes.write_all(&line).await.map_err(|_| LinkDown)?;
        side.writes.flush().await.map_err(|_| LinkDown)
    }

    /// The next message that crossed to `end`, once it has; `None` while the link is down, or
    /// once it went down with nothing more for `end`.
    pub async fn receive(&mut self, end: End) -> Option<M> {
        self.side(end)?.inbox.recv().await
    }

    /// Every message that crossed, on every link this was, in the order each was read.
    pub fn crossed(&self) -> Vec<Crossed<M>> {
        self.record
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// How many of the messages that crossed, either way, `which` matches.
    pub fn times_crossed(&self, which: impl Fn(&M) -> bool) -> usize {
        self.record
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|crossed| which(&crossed.message))
            .count()
    }

    fn side(&mut self, end: End) -> Option<&mut Side<M>> {
        let joined = self.joined.as_mut()?;
        Some(match end {
            End::Desktop => &mut joined.desktop,
            End::Runner => &mut joined.runner,
        })
    }
}

/// Reads each line one end's far side is sent, records it as crossed from `from`, and hands
/// it to that end.
async fn reader<M: DeserializeOwned + Clone>(
    reads: tokio::io::ReadHalf<DuplexStream>,
    from: End,
    to: mpsc::UnboundedSender<M>,
    record: Arc<Mutex<Vec<Crossed<M>>>>,
) {
    let mut lines = BufReader::new(reads).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let message: M = serde_json::from_str(&line).expect("a line the link was sent");
        record
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Crossed {
                from,
                message: message.clone(),
            });
        if to.send(message).is_err() {
            return;
        }
    }
}
