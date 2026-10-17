// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! How every test in this folder starts the binary (#1284): with none of the variables that
//! choose a project, which a suite run inside a chat inherits under both names (V93k). The test
//! sets its own after. `every_test_starts_the_binary_unsteered.rs` fails on a test that starts
//! one any other way.
//!
//! Each test file declares `mod support;` and uses what it needs, so the rest is unused there.
#![allow(dead_code)]

use std::ffi::OsStr;
use std::process::Command;

/// The `purlis` binary cargo built for this run. Its path, for a test that writes it into a
/// hook or hands it to a shell: such a file starts every process through [`command`].
pub const PURLIS: &str = env!("CARGO_BIN_EXE_purlis");

/// The `charter` alias cargo built for this run (RN-3), which runs the `purlis` beside it.
pub const CHARTER: &str = env!("CARGO_BIN_EXE_charter");

/// `purlis`, unsteered.
pub fn purlis() -> Command {
    command(PURLIS)
}

/// `program`, unsteered: every spelling of every variable that chooses a project
/// (`purlis_core::envvar::SELECTING`) removed from what it inherits.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    for rest in purlis_core::envvar::SELECTING {
        for spelling in purlis_core::envvar::spellings(&format!("PURLIS_{rest}")) {
            command.env_remove(spelling);
        }
    }
    command
}

/// Where the app binds its hooks socket in a project at `root`: `.purlis/app/hooks.sock`, its
/// folder made. A hook spools a line the app does not take in time beside its socket, and only
/// under a project's state folder, the one a sandboxed chat is denied (`spool::covered`, V63).
/// A test whose socket sits anywhere else loses that line, and a loaded runner loses it
/// (#2255, #2386).
#[cfg(unix)]
pub fn hooks_socket(root: &std::path::Path) -> std::path::PathBuf {
    // RED (#2386, temporary): the old location, outside a covered spool.
    let socket = root.join("hooks.sock");
    std::fs::create_dir_all(socket.parent().expect("a folder")).expect("the socket's folder");
    socket
}

/// What a host listening at `socket` was told of one kind of line, once the hook that told it
/// has exited: the lines the hook spooled beside the socket ([`spooled`]), then those the
/// host's hearer recorded live into `live`.
///
/// **Nothing is waited for.** A hook exits once the host has said it recorded the line, which
/// is after its hearer ran, or once the line is durable in the spool (`hookwire::deliver`,
/// FD-30). **The spool is drained first**: a line the host recorded late, after its hook gave
/// up and spooled it, the drain knows and skips (#952), and its hearer has run by then. Read
/// the other way round, a late hearer could run between the two reads and the line be in
/// neither.
///
/// **Each distinct line once.** A line can be in both where the host was still recording it
/// live as the drain ran: the drain waits for that a while, then hands it on too, erring on a
/// line twice, never none (#952). Neither copy carries the delivery id that tells them apart,
/// so here they are told apart by what they say.
#[cfg(unix)]
pub fn told<T: PartialEq>(
    socket: &std::path::Path,
    live: &std::sync::mpsc::Receiver<T>,
    kind: impl Fn(purlis_core::hookwire::spool::Spooled) -> Option<T>,
) -> Vec<T> {
    let mut told = spooled(socket, kind);
    told.extend(live.try_iter());
    distinct(told)
}

/// `lines`, each distinct one once, in the order first seen.
pub fn distinct<T: PartialEq>(lines: Vec<T>) -> Vec<T> {
    let mut once = Vec::new();
    for line in lines {
        if !once.contains(&line) {
            once.push(line);
        }
    }
    once
}

/// The lines of one kind spooled beside the host's `socket`, drained. `kind` picks the line
/// out of a spooled one.
///
/// Panics on a line that did not check, a gap, or a line of another kind: never the quiet
/// answer a test reads as "nothing was spooled".
#[cfg(unix)]
pub fn spooled<T>(
    socket: &std::path::Path,
    kind: impl Fn(purlis_core::hookwire::spool::Spooled) -> Option<T>,
) -> Vec<T> {
    use purlis_core::hookwire::spool;
    let mut spooled = Vec::new();
    spool::drain(&spool::dir_for(socket), &mut |item| {
        match item {
            spool::Drained::Line { line, .. } => {
                let said = format!("{line:?}");
                spooled
                    .push(kind(line).unwrap_or_else(|| {
                        panic!("the spool held a line of another kind: {said}")
                    }));
            }
            spool::Drained::Spool { .. } => {}
            other => panic!("the spool held more than lines that check: {other:?}"),
        }
        Ok(())
    })
    .expect("the spool drains");
    spooled
}
