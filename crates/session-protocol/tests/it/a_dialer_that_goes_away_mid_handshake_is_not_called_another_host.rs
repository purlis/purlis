// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A desktop that gives up on a link part way through its handshake (its own wait ran out, or
//! it quit) closes the stream. The runner says so: that the other end went away before the link
//! was made, never that it is not the host the link was paired with, which sends the person
//! looking for a changed key that never changed.

use std::pin::Pin;
use std::task::{Context, Poll};

use purlis_session_protocol::hostlink::{self, Handshake, LinkRefused, Liveness};
use tokio::io::{AsyncRead, AsyncWrite, DuplexStream, ReadBuf, duplex};

use crate::common;
use common::hosts::{a_as_pinned, b_as_pinned, host_a, host_b};

/// The dialer's end of a stream that is dropped once the dialer has read the answering host's
/// hello and goes on to write its half of the handshake: a dialer gone mid-handshake.
struct GoesAwayAfterTheHello {
    io: Option<DuplexStream>,
    read_any: bool,
}

impl AsyncRead for GoesAwayAfterTheHello {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = &mut *self;
        let Some(io) = this.io.as_mut() else {
            return Poll::Ready(Err(std::io::ErrorKind::BrokenPipe.into()));
        };
        let before = buf.filled().len();
        let polled = Pin::new(io).poll_read(cx, buf);
        if buf.filled().len() > before {
            this.read_any = true;
        }
        polled
    }
}

impl AsyncWrite for GoesAwayAfterTheHello {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = &mut *self;
        if this.read_any {
            this.io = None;
        }
        match this.io.as_mut() {
            Some(io) => Pin::new(io).poll_write(cx, buf),
            None => Poll::Ready(Err(std::io::ErrorKind::BrokenPipe.into())),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.io.as_mut() {
            Some(io) => Pin::new(io).poll_flush(cx),
            None => Poll::Ready(Ok(())),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.io.as_mut() {
            Some(io) => Pin::new(io).poll_shutdown(cx),
            None => Poll::Ready(Ok(())),
        }
    }
}

#[tokio::test]
async fn a_dialer_gone_mid_handshake_is_said_to_have_gone_and_not_to_be_another_host() {
    let (a_end, b_end) = duplex(1 << 16);
    let answered = tokio::spawn(async move {
        hostlink::answer(
            b_end,
            Handshake::THIS_BUILD,
            &host_b(),
            &a_as_pinned(),
            Liveness::default(),
        )
        .await
        .map(|_| ())
    });
    let going = GoesAwayAfterTheHello {
        io: Some(a_end),
        read_any: false,
    };
    let dialed = hostlink::connect(
        going,
        Handshake::THIS_BUILD,
        &host_a(),
        &b_as_pinned(),
        Liveness::default(),
    )
    .await;
    assert!(dialed.is_err(), "the dialer went away");

    let answered = answered.await.unwrap().unwrap_err();
    assert!(
        matches!(answered, LinkRefused::WentAway),
        "{answered:?}: {answered}"
    );
    let said = answered.to_string();
    assert!(!said.contains("not the host"), "{said}");
    assert!(
        said.contains("closed the stream before the link was made"),
        "{said}"
    );
}
