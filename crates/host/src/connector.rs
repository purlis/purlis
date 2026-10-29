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

/// **Why a check refused a connection** (#3292): in words, or because the far end's key is no
/// longer the one it was paired with, which the link tells as a changed key
/// ([`purlis_session_protocol::hostlink::KeyNoLongerHeld`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotChecked {
    Refused(String),
    KeyChanged(String),
}

/// [`BeforeEach`], whose refusal can say the far end's key changed ([`NotChecked`]).
pub type BeforeEachChecked = Arc<dyn Fn() -> Result<(), NotChecked> + Send + Sync>;

/// What each run of the command is given, made just before it runs: its variables, those it is
/// not handed from this process, and what has to be kept until it has ended (a folder made for
/// it). `Err` with why, in words, refuses the connection.
pub type Prepare = Arc<dyn Fn() -> Result<Prepared, String> + Send + Sync>;

/// What [`Prepare`] made for one run.
pub struct Prepared {
    /// Each variable and its value.
    pub env: Vec<(std::ffi::OsString, std::ffi::OsString)>,
    /// The variables not handed on from this process.
    pub cleared: Vec<std::ffi::OsString>,
    /// Kept until the run has ended.
    pub held: Box<dyn Send + Sync>,
}

/// A connector that runs `program` with `args` for each connection.
#[derive(Clone)]
pub struct CommandConnector {
    program: std::path::PathBuf,
    args: Vec<String>,
    before_each: BeforeEachChecked,
    prepare: Option<Prepare>,
}

impl CommandConnector {
    pub fn new(
        program: impl Into<std::path::PathBuf>,
        args: Vec<String>,
        before_each: BeforeEach,
    ) -> CommandConnector {
        CommandConnector::checked(
            program,
            args,
            Arc::new(move || before_each().map_err(NotChecked::Refused)),
        )
    }

    /// [`CommandConnector::new`], with a check that can refuse a far end as its key changed
    /// ([`NotChecked::KeyChanged`], #3292).
    pub fn checked(
        program: impl Into<std::path::PathBuf>,
        args: Vec<String>,
        before_each: BeforeEachChecked,
    ) -> CommandConnector {
        CommandConnector {
            program: program.into(),
            args,
            before_each,
            prepare: None,
        }
    }

    /// This connector, with each run given what `prepare` makes for it.
    #[must_use]
    pub fn preparing(mut self, prepare: Prepare) -> CommandConnector {
        self.prepare = Some(prepare);
        self
    }
}

/// One connection: the command's standard output to read and its standard input to write,
/// closed when this end shuts down, so the command reads its end. The command is killed when
/// this is dropped.
pub struct Connected {
    _child: Child,
    _held: Option<Box<dyn Send + Sync>>,
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
                .map_err(|why| match why {
                    NotChecked::Refused(why) => std::io::Error::other(why),
                    NotChecked::KeyChanged(why) => std::io::Error::other(
                        purlis_session_protocol::hostlink::KeyNoLongerHeld(why),
                    ),
                })?;
            let prepared = match this.prepare.clone() {
                Some(prepare) => Some(
                    tokio::task::spawn_blocking(move || prepare())
                        .await
                        .map_err(std::io::Error::other)?
                        .map_err(std::io::Error::other)?,
                ),
                None => None,
            };
            let mut command = tokio::process::Command::new(&this.program);
            let held = prepared.map(|prepared| {
                command.envs(prepared.env);
                for name in &prepared.cleared {
                    command.env_remove(name);
                }
                prepared.held
            });
            let mut child = command
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
                _held: held,
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
