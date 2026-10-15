// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The dialing host keeps its link to another host up: it links again on its own when the link
//! ends, and links anew ahead of a limit on a stream's life.

use std::time::Duration;

use futures::future::BoxFuture;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
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
    /// A link is up: the session protocol on it, from its start. A link that was up before
    /// ends once this one is up, when it was renewed ahead of its limit. Boxed: a link is
    /// large, and a word of why is small.
    Up(Box<Link>),
    /// The link is down, or did not come up, and why: the host is linking again.
    Down(String),
}

/// A kept link's [`Linked`]s. Dropping it lets the link go.
pub struct Links(mpsc::Receiver<Linked>);

impl Links {
    /// What happened next, or `None` once the link has been let go of.
    pub async fn next(&mut self) -> Option<Linked> {
        self.0.recv().await
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
    tokio::spawn(async move {
        let dial = || async {
            let io = connector
                .connect()
                .await
                .map_err(|why| format!("the connector did not connect: {why}"))?;
            connect_kept(io, handshake, &ours, &answering, liveness)
                .await
                .map_err(|why| why.to_string())
        };
        let mut pause = FIRST_PAUSE;
        loop {
            let (link, mut ends) = match dial().await {
                Ok(linked) => linked,
                Err(why) => {
                    if tell.send(Linked::Down(why)).await.is_err() {
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
            let Some(why) = hold(&dial, &tell, &mut ends, liveness).await else {
                ends.retire.abort();
                return;
            };
            if tell.send(Linked::Down(why)).await.is_err() {
                return;
            }
        }
    });
    Links(told)
}

/// Hold a link that is up until it ends, renewing it ahead of its limit; `None` once nobody
/// listens. A renewal that fails keeps the link it would have replaced, and tries again after a
/// pause.
async fn hold<F, Fut>(
    dial: &F,
    tell: &mpsc::Sender<Linked>,
    ends: &mut Ends,
    liveness: Liveness,
) -> Option<String>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<(Link, Ends), String>>,
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
                    ends.retire.abort();
                    *ends = renewed;
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
