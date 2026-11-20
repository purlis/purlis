// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The endpoint a chat's forge traffic reaches, made for that chat alone.
//!
//! **Who is asking is the endpoint's to say.** Each chat has an endpoint of its own, and the chat
//! of a connection is the chat its endpoint was bound for ([`ChatEndpoint::chat`]). Nothing a
//! request carries chooses it. The sentinel is the second lock: a process that reached a chat's
//! endpoint without being that chat still holds nothing that opens it.
//!
//! Two transports, behind one trait:
//!
//! - **A unix socket** ([`UnixEndpoint`]), in a folder only the app can enter, mode 0600: the
//!   kernel's file permissions say who may connect, and nothing is listening on the network. Used
//!   wherever the client can speak to one.
//! - **A loopback port** ([`LoopbackEndpoint`]): a TCP listener on `127.0.0.1`, an ephemeral port
//!   bound for one chat. Any local process can connect to it, so here the sentinel is the only
//!   lock, and ADR 0128 Open item 2 (reading the connecting process from the socket table) is
//!   what would add the other. Used where the client cannot use a unix socket (git's HTTPS
//!   remotes).
//!
//! This slice binds and accepts; the connection loop that reads a request and calls
//! [`ForgeProxy::handle`](super::ForgeProxy::handle) is the listener slice's.

use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use super::sentinel::ChatId;

/// Where an endpoint listens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointKind {
    UnixSocket(PathBuf),
    Loopback(SocketAddr),
}

/// How a client speaks to an endpoint. The policy is the same for both ([`ChatPolicy`]
/// (super::ChatPolicy)): only how a request arrives differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientProtocol {
    /// Plain HTTP in origin form, on a unix socket: `gh` (its `http_unix_socket` setting, with a
    /// host name of its own). The request names the forge by `Host`, which the policy matches.
    PlainHttp,
    /// `CONNECT host:port`, then TLS: git and `glab` through `HTTPS_PROXY`. The proxy asks
    /// [`ChatPolicy::connect`](super::ChatPolicy::connect), terminates TLS with a leaf of the
    /// chat's own CA for a host it says to, and decides each decrypted request as any other.
    ConnectTls,
}

/// One chat's endpoint.
pub trait ChatEndpoint: Send {
    /// How its clients speak to it. A unix socket serves [`ClientProtocol::PlainHttp`]; a loopback
    /// port is the `HTTPS_PROXY` of git and `glab`, which is [`ClientProtocol::ConnectTls`].
    fn protocol(&self) -> ClientProtocol;
    /// The chat this endpoint was bound for: the only source of a connection's chat.
    fn chat(&self) -> &ChatId;
    /// Where it listens, for the chat's environment and git configuration.
    fn kind(&self) -> EndpointKind;
}

/// Which transport to bind for a client: a unix socket where the client can use one, else a
/// loopback port.
pub fn pick_endpoint(
    chat: &ChatId,
    socket_dir: &Path,
    client_can_use_a_unix_socket: bool,
) -> io::Result<Box<dyn ChatEndpoint>> {
    #[cfg(unix)]
    if client_can_use_a_unix_socket {
        return Ok(Box::new(UnixEndpoint::bind(chat, socket_dir)?));
    }
    let _ = (socket_dir, client_can_use_a_unix_socket);
    Ok(Box::new(LoopbackEndpoint::bind(chat)?))
}

/// A TCP listener on `127.0.0.1`, for one chat.
pub struct LoopbackEndpoint {
    chat: ChatId,
    listener: TcpListener,
}

impl LoopbackEndpoint {
    /// Bind an ephemeral port on the loopback address, never `0.0.0.0`.
    pub fn bind(chat: &ChatId) -> io::Result<Self> {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
        Ok(Self {
            chat: chat.clone(),
            listener,
        })
    }

    /// The next connection, and the chat it is for: this endpoint's.
    pub fn accept(&self) -> io::Result<(ChatId, TcpStream)> {
        let (stream, _) = self.listener.accept()?;
        Ok((self.chat.clone(), stream))
    }
}

impl ChatEndpoint for LoopbackEndpoint {
    fn protocol(&self) -> ClientProtocol {
        ClientProtocol::ConnectTls
    }

    fn chat(&self) -> &ChatId {
        &self.chat
    }

    fn kind(&self) -> EndpointKind {
        EndpointKind::Loopback(
            self.listener
                .local_addr()
                .unwrap_or_else(|_| SocketAddr::from((Ipv4Addr::LOCALHOST, 0))),
        )
    }
}

#[cfg(unix)]
pub use unix::UnixEndpoint;

#[cfg(unix)]
mod unix {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::os::unix::net::{UnixListener, UnixStream};

    /// A unix socket in a folder of mode 0700, for one chat. The socket's name is random, so it
    /// is not a name a chat could guess for another's, and the file is removed with the endpoint.
    pub struct UnixEndpoint {
        chat: ChatId,
        listener: UnixListener,
        path: PathBuf,
    }

    impl UnixEndpoint {
        /// Bind in `dir` (made, mode 0700, if it is not there). A socket path is short on macOS
        /// (104 bytes): keep `dir` short.
        pub fn bind(chat: &ChatId, dir: &Path) -> io::Result<Self> {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)?;
            let mut name = [0u8; 8];
            getrandom::fill(&mut name).map_err(|e| io::Error::other(e.to_string()))?;
            let path = dir.join(format!("fp-{}.sock", data_encoding::HEXLOWER.encode(&name)));
            let listener = UnixListener::bind(&path)?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            Ok(Self {
                chat: chat.clone(),
                listener,
                path,
            })
        }

        /// The next connection, and the chat it is for: this endpoint's.
        pub fn accept(&self) -> io::Result<(ChatId, UnixStream)> {
            let (stream, _) = self.listener.accept()?;
            Ok((self.chat.clone(), stream))
        }
    }

    impl ChatEndpoint for UnixEndpoint {
        fn protocol(&self) -> ClientProtocol {
            ClientProtocol::PlainHttp
        }

        fn chat(&self) -> &ChatId {
            &self.chat
        }

        fn kind(&self) -> EndpointKind {
            EndpointKind::UnixSocket(self.path.clone())
        }
    }

    impl Drop for UnixEndpoint {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
