// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A connector is a command, held as data** (ADR 0078 §2, #2044): an argument vector run
//! directly, never through a shell, whose standard input and output are a new byte stream to
//! the other host's bridge for each connection. Before each run, a check the connector was made
//! with decides whether to run it at all (a Docker pool's: that the runner's container is still
//! confined as it was made), and a refusal runs nothing. What the command says on standard
//! error goes to this host's log, since that is where a connector explains why it failed; purlis
//! never reads it to decide anything.

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::future::BoxFuture;
use purlis_session_protocol::hostlink::Connector;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::process::{Child, ChildStdin, ChildStdout};

/// The check made before each connection: `Err` with why, in words, refuses it.
pub type BeforeEach = Arc<dyn Fn() -> Result<(), String> + Send + Sync>;

/// A connector that runs `program` with `args` for each connection.
#[derive(Clone)]
pub struct CommandConnector {
    program: std::path::PathBuf,
    args: Vec<String>,
    before_each: BeforeEach,
}

impl CommandConnector {
    pub fn new(
        program: impl Into<std::path::PathBuf>,
        args: Vec<String>,
        before_each: BeforeEach,
    ) -> CommandConnector {
        CommandConnector {
            program: program.into(),
            args,
            before_each,
        }
    }
}

/// One connection: the command's standard output to read and its standard input to write,
/// closed when this end shuts down, so the command reads its end. The command is killed when
/// this is dropped.
pub struct Connected {
    _child: Child,
    stdout: ChildStdout,
    stdin: Option<ChildStdin>,
}

impl Connector for CommandConnector {
    type Io = Connected;

    fn connect(&self) -> BoxFuture<'static, std::io::Result<Connected>> {
        let this = self.clone();
        Box::pin(async move {
            let checked = this.before_each.clone();
            tokio::task::spawn_blocking(move || checked())
                .await
                .map_err(std::io::Error::other)?
                .map_err(std::io::Error::other)?;
            let mut child = tokio::process::Command::new(&this.program)
                .args(&this.args)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::inherit())
                .kill_on_drop(true)
                .spawn()?;
            let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
                return Err(std::io::Error::other("the connector's stdio was not piped"));
            };
            Ok(Connected {
                _child: child,
                stdout,
                stdin: Some(stdin),
            })
        })
    }
}

impl AsyncRead for Connected {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdout).poll_read(cx, buf)
    }
}

impl AsyncWrite for Connected {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.stdin.as_mut() {
            Some(stdin) => Pin::new(stdin).poll_write(cx, buf),
            None => Poll::Ready(Err(std::io::ErrorKind::BrokenPipe.into())),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.stdin.as_mut() {
            Some(stdin) => Pin::new(stdin).poll_flush(cx),
            None => Poll::Ready(Ok(())),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let flushed = match self.stdin.as_mut() {
            Some(stdin) => Pin::new(stdin).poll_flush(cx),
            None => return Poll::Ready(Ok(())),
        };
        if flushed.is_ready() {
            // Dropping the pipe's end is what closes it: the command reads its end.
            self.stdin = None;
        }
        flushed
    }
}
