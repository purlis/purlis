// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0
//
// The shape of this client (connect with retries, keep the companion alive with pings, and on a
// crash ping before asking for the dump) follows minidumper-child's `client.rs`
// (https://github.com/timfish/minidumper-child, at 7500498), copyright the minidumper-child
// authors, licensed MIT OR Apache-2.0, and used here under Apache-2.0.

//! **The app's side**: a `minidumper` client connected to the companion, and EmbarkStudios'
//! `CrashHandler` attached to this process, which on a crash asks the companion for a dump.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crash_handler::{CrashContext, CrashEvent, CrashEventResult, CrashHandler};
use minidumper::{Client, SocketName};

/// How long the app waits for the companion to bind its socket.
const CONNECT_FOR: Duration = Duration::from_secs(3);

/// The watch, held by [`crate::Watching`]. Dropping it detaches the handler, and the companion,
/// no longer pinged, exits.
pub(crate) struct Attached {
    pub(crate) client: Arc<Client>,
    _handler: CrashHandler,
    _companion: std::process::Child,
}

/// Connects to the companion `companion` serves at `socket`, and attaches the crash handler.
pub(crate) fn attach(
    socket: &Path,
    mut companion: std::process::Child,
) -> Result<Attached, minidumper::Error> {
    let client = match connect(socket) {
        Ok(client) => Arc::new(client),
        Err(why) => {
            // A companion that already ended could not bind the socket: say that, not only
            // that the socket is missing.
            let ended = companion.try_wait().ok().flatten();
            let _ = companion.kill();
            let _ = companion.wait();
            return Err(match ended {
                Some(status) => minidumper::Error::Io(std::io::Error::other(format!(
                    "the companion ended ({status}) before it served: {why}"
                ))),
                None => why,
            });
        }
    };
    let pinging = Arc::clone(&client);
    let _ = std::thread::Builder::new()
        .name("crash-handler-ping".into())
        .spawn(move || {
            loop {
                std::thread::sleep(crate::STALE_AFTER / 2);
                if pinging.ping().is_err() {
                    break;
                }
            }
        });
    let handler = match CrashHandler::attach(on_crash(Arc::clone(&client))) {
        Ok(handler) => handler,
        Err(why) => {
            let _ = companion.kill();
            let _ = companion.wait();
            return Err(minidumper::Error::Io(std::io::Error::other(why)));
        }
    };
    // On Linux only the companion may read this process when it crashes.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    handler.set_ptracer(Some(companion.id()));
    Ok(Attached {
        client,
        _handler: handler,
        _companion: companion,
    })
}

/// A client of the companion at `socket`, once it has bound it, within [`CONNECT_FOR`].
fn connect(socket: &Path) -> Result<Client, minidumper::Error> {
    let until = std::time::Instant::now() + CONNECT_FOR;
    loop {
        match Client::with_name(SocketName::Path(socket)) {
            Ok(client) => return Ok(client),
            Err(_) if std::time::Instant::now() < until => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(why) => return Err(why),
        }
    }
}

/// What the handler does on a crash: ask the companion for a dump of this process.
///
/// The workspace's fifth audited `unsafe` block (the operator's ruling of 2026-11-10, ADR 0083
/// as amended that day; `crates/purlis-core/tests/it/five_unsafe_blocks.rs`): `crash-handler`
/// takes its callback only from the `unsafe fn` `make_crash_event`.
#[allow(
    unsafe_code,
    reason = "the fifth audited block, crash-handler's make_crash_event; see the SAFETY comment"
)]
fn on_crash(client: Arc<Client>) -> Box<dyn CrashEvent> {
    // SAFETY: `make_crash_event` is `unsafe` because its closure runs in a crashed process: on
    // macOS on the handler's own thread with every other thread suspended, on Linux in a signal
    // handler. So it must not take a lock another thread may hold, allocate, or unwind. This one
    // makes two calls on a client connected before the crash: a ping and `request_dump`, each a
    // write to and a read from an open socket (a Mach message on macOS), neither taking a lock or
    // allocating, as minidumper documents for `request_dump`. It never panics: both errors are
    // values, and `Handled(false)` passes the crash on to the system's own handler.
    unsafe {
        crash_handler::make_crash_event(move |crash: &CrashContext| {
            // The companion answers a ping only once it has handled every message before it, so
            // a `panicked` sent just before the crash is heard before the dump is written.
            let _ = client.ping();
            CrashEventResult::Handled(client.request_dump(crash).is_ok())
        })
    }
}
