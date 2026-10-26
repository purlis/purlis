// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis listen PORT -- COMMAND…`** (#2019): runs a server for a sandboxed chat on the port
//! it was allowed to serve on.
//!
//! A chat allowed to serve on a port never listens on a TCP port of this machine: its server
//! listens on a socket of its own, in the folder purlis made for the chat
//! ([`purlis_core::sandbox::door::FOLDER_ENV`]), and purlis's door on the port carries every
//! request to it. This runs `COMMAND` with what it needs to do so:
//!
//! - `PURLIS_SERVE_SOCKET`, the socket to listen on, and `PURLIS_SERVE_PORT`;
//! - `PORT` and `HOST=127.0.0.1`, which many servers read, so what one prints is the door's;
//! - for Node, the shim (`listen/shim.cjs`) on `NODE_OPTIONS`, which moves a server started on
//!   the port onto the socket and has it say it listens on the port: Vite, Next.js, webpack and
//!   Express, unchanged.
//!
//! In a chat that may not serve yet, `COMMAND` runs as it is, with `PORT` and `HOST`: its server
//! is refused the port, and purlis asks the person once.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The shim, as `purlis listen` writes it for Node to load.
pub const SHIM: &str = include_str!("listen/shim.cjs");

/// The variables a server is handed the socket and the port by.
pub const SOCKET_ENV: &str = "PURLIS_SERVE_SOCKET";
pub const PORT_ENV: &str = "PURLIS_SERVE_PORT";

/// **What `COMMAND` is started with**, for `port` in a chat whose servers listen in `folder`
/// (none where it may not serve yet), the shim at `shim`, and `node_options` as they were: each
/// variable and its value.
pub fn env(
    port: u16,
    folder: Option<&Path>,
    shim: Option<&Path>,
    node_options: Option<&str>,
) -> Vec<(String, String)> {
    let mut out = vec![
        ("PORT".to_owned(), port.to_string()),
        ("HOST".to_owned(), "127.0.0.1".to_owned()),
    ];
    let Some(folder) = folder else {
        return out;
    };
    let socket = purlis_core::sandbox::localport::serve_socket(folder, port);
    out.push((SOCKET_ENV.to_owned(), socket.display().to_string()));
    out.push((PORT_ENV.to_owned(), port.to_string()));
    if let Some(shim) = shim {
        let quoted = shim
            .display()
            .to_string()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let required = format!("--require \"{quoted}\"");
        out.push((
            "NODE_OPTIONS".to_owned(),
            match node_options.filter(|options| !options.trim().is_empty()) {
                Some(options) => format!("{required} {options}"),
                None => required,
            },
        ));
    }
    out
}

/// Where the shim is written for a run: the chat's own temp folder.
fn shim_path() -> PathBuf {
    std::env::temp_dir().join("purlis-listen-shim.cjs")
}

/// Runs `command` as the module says, in place of this process; its error where it could not.
pub fn run(port: u16, command: &[OsString]) -> ExitCode {
    let Some((program, args)) = command.split_first() else {
        crate::voice::err("purlis listen runs a server: name its command after --.");
        return ExitCode::from(2);
    };
    if port == 0 {
        crate::voice::err(
            "purlis listen runs a server on one port, a number from 1 to 65535: name it.",
        );
        return ExitCode::from(2);
    }
    let folder = purlis_core::envvar::var(purlis_core::sandbox::door::FOLDER_ENV)
        .filter(|folder| !folder.is_empty())
        .map(PathBuf::from);
    let shim = folder.as_ref().and_then(|_| {
        let path = shim_path();
        std::fs::write(&path, SHIM).ok().map(|()| path)
    });
    let node_options = purlis_core::envvar::var("NODE_OPTIONS");
    let mut child = std::process::Command::new(program);
    child.args(args);
    for (name, value) in env(
        port,
        folder.as_deref(),
        shim.as_deref(),
        node_options.as_deref(),
    ) {
        child.env(name, value);
    }
    use std::os::unix::process::CommandExt;
    let why = child.exec();
    crate::voice::err(format!(
        "purlis listen could not run {}: {why}",
        program.to_string_lossy()
    ));
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests;
