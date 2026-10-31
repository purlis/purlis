// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The dialing host keeps its link to another host up: it links again on its own when the link
//! ends, and links anew ahead of a limit on a stream's life. A link renewed ahead of its limit
//! ends once whoever reads the kept link has moved to the new one ([`Links::moved`]), or once
//! the ping's deadline has passed, so what was in flight on it is answered first.

use std::time::Duration;

use futures::future::BoxFuture;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, watch};
use tokio::time::Instant;

use super::{Handshake, Identity, Liveness, Pinned, channel::Ends, connect_kept};
use crate::link::Link;

/// How a dialing host reaches the other host's link: a connector (ADR 0078 §2), whose every
/// connection is a new byte stream to it.
pub trait Connector: Send + Sync + 'static {
    type Io: AsyncRead + AsyncWrite + Unpin + Send + 'static;

    /// A new connection, or why there is none.
    fn connect(&self) -> BoxFuture<'static, std::io::Result<Self::Io>>;
}

/// What happens to a kept link, in order.
pub enum Linked {
    /// A link is up: the session protocol on it, from its start. A link that was up before,
    /// renewed ahead of its limit, ends once the reader says it has moved to this one
    /// ([`Links::moved`]), or after [`Liveness::deadline`] at the latest. Boxed: a link is
    /// large, and a word of why is small.
    Up(Box<Link>),
    /// The link is down, or did not come up, and why: the host is linking again.
    Down(Down),
}

/// Why a kept link is down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Down {
    /// In words a person reads.
    pub why: String,
    /// The host at the far end answered with another key than the one pinned at its pairing
    /// (#2679): it is refused on every try, so it is not linked again until the person removes
    /// it and adds it anew.
    pub key_changed: bool,
    /// The far end is gone for good (#4149): a Docker runner whose container Docker no longer
    /// lists, removed outside purlis. The kept link is not tried again; a new one is taken only
    /// when something asks for it, and its check says so again.
    pub gone: bool,
}

impl Down {
    /// Down for `why`, with the same key at the far end, as far as anyone knows.
    pub fn because(why: impl Into<String>) -> Down {
        Down {
            why: why.into(),
            key_changed: false,
            gone: false,
        }
    }
}

impl std::fmt::Display for Down {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.why)
    }
}

/// **A connector's refusal of a far end whose key is no longer the one pinned at its pairing**
/// (#3292), in words a person reads: a connector's `connect` that fails with this, inside its
/// [`std::io::Error`], is told as a [`Down`] whose key changed, as a handshake answered with
/// another key is. A cluster runner's connector refuses so when its operator no longer vouches
/// for the key it was paired with.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct KeyNoLongerHeld(pub String);

/// **A connector's refusal of a far end that is gone for good** (#4149), in words a person
/// reads: a connector's `connect` that fails with this, inside its [`std::io::Error`], is told as
/// a [`Down`] that is [`Down::gone`], and the kept link ends there rather than trying again. A
/// Docker runner's connector refuses so when Docker no longer lists the runner's container.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct FarEndGone(pub String);

/// A kept link's [`Linked`]s. Dropping it lets the link go.
pub struct Links {
    told: mpsc::Receiver<Linked>,
    /// How many links have been read up.
    ups: u64,
    /// How many of them the reader has moved to.
    moved: watch::Sender<u64>,
}

impl Links {
    /// What happened next, or `None` once the link has been let go of.
    pub async fn next(&mut self) -> Option<Linked> {
        let linked = self.told.recv().await;
        if let Some(Linked::Up(_)) = &linked {
            self.ups += 1;
        }
        linked
    }

    /// The reader has moved what it does to the last link it was told is up: the link that
    /// was up before it, renewed ahead of its limit, ends now rather than at the deadline.
    pub fn moved(&self) {
        self.moved.send_replace(self.ups);
    }
}

/// The first pause before linking again after a link would not come up; it doubles with each
/// try that fails, up to [`MOST_PAUSE`].
pub const FIRST_PAUSE: Duration = Duration::from_millis(250);

/// The longest pause between tries.
pub const MOST_PAUSE: Duration = Duration::from_secs(30);

/// Keep a link to `answering`, the host pinned at pairing, through `connector`, as `ours`:
/// link now, again whenever the link ends, and anew every [`Liveness::renew_after`]. Each
/// link that comes up, and each time one is down, is told in order on the returned [`Links`].
pub fn keep_linked<C: Connector>(
    connector: C,
    handshake: Handshake,
    ours: Identity,
    answering: Pinned,
    liveness: Liveness,
) -> Links {
    let (tell, told) = mpsc::channel(4);
    let (moved, moved_to) = watch::channel(0);
    tokio::spawn(async move {
        let mut ups: u64 = 0;
        let dial = || async {
            let io = connector.connect().await.map_err(|why| Down {
                // A connector's own check that the far end's key is no longer the one
                // it was paired with (a cluster runner's, #3292) is a changed key too.
                key_changed: why
                    .get_ref()
                    .is_some_and(|inner| inner.is::<KeyNoLongerHeld>()),
                gone: why.get_ref().is_some_and(|inner| inner.is::<FarEndGone>()),
                why: format!("the connector did not connect: {why}"),
            })?;
            connect_kept(io, handshake, &ours, &answering, liveness)
                .await
                .map_err(|why| Down {
                    key_changed: matches!(why, super::LinkRefused::NotThePinnedHost { .. }),
                    gone: false,
                    why: why.to_string(),
                })
        };
        let mut pause = FIRST_PAUSE;
        loop {
            let (link, mut ends) = match dial().await {
                Ok(linked) => linked,
                Err(why) => {
                    // A far end gone for good is said once, and the kept link ends.
                    let gone = why.gone;
                    if tell.send(Linked::Down(why)).await.is_err() || gone {
                        return;
                    }
                    tokio::time::sleep(pause).await;
                    pause = (pause * 2).min(MOST_PAUSE);
                    continue;
                }
            };
            pause = FIRST_PAUSE;
            if tell.send(Linked::Up(Box::new(link))).await.is_err() {
                ends.retire.abort();
                return;
            }
            ups += 1;
            let Some(why) = hold(
                &dial,
                &tell,
                &mut ends,
                liveness,
                &mut ups,
                moved_to.clone(),
            )
            .await
            else {
                ends.retire.abort();
                return;
            };
            if tell.send(Linked::Down(Down::because(why))).await.is_err() {
                return;
            }
        }
    });
    Links {
        told,
        ups: 0,
        moved,
    }
}

/// Hold a link that is up until it ends, renewing it ahead of its limit; `None` once nobody
/// listens. A renewal that fails keeps the link it would have replaced, and tries again after a
/// pause. A renewal that comes up ends the link it replaces once the reader has moved to the
/// new one (`moved_to` reaches its count in `ups`), or after the ping's deadline.
async fn hold<F, Fut>(
    dial: &F,
    tell: &mpsc::Sender<Linked>,
    ends: &mut Ends,
    liveness: Liveness,
    ups: &mut u64,
    mut moved_to: watch::Receiver<u64>,
) -> Option<String>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<(Link, Ends), Down>>,
{
    let mut renew_at = liveness.renew_after.map(|after| Instant::now() + after);
    let mut pause = FIRST_PAUSE;
    loop {
        let renewal = async {
            match renew_at {
                Some(at) => tokio::time::sleep_until(at).await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            why = &mut ends.ended => {
                return Some(why.unwrap_or_else(|_| "the link was ended".to_owned()));
            }
            () = tell.closed() => return None,
            () = renewal => match dial().await {
                Ok((link, renewed)) => {
                    tell.send(Linked::Up(Box::new(link))).await.ok()?;
                    *ups += 1;
                    let this = *ups;
                    let replaced = std::mem::replace(ends, renewed);
                    tokio::select! {
                        _ = moved_to.wait_for(|moved| *moved >= this) => {}
                        () = tokio::time::sleep(liveness.deadline) => {}
                        () = tell.closed() => {}
                    }
                    replaced.retire.abort();
                    pause = FIRST_PAUSE;
                    renew_at = liveness.renew_after.map(|after| Instant::now() + after);
                }
                Err(_) => {
                    renew_at = Some(Instant::now() + pause);
                    pause = (pause * 2).min(MOST_PAUSE);
                }
            },
        }
    }
}
