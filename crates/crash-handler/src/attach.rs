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
    // No keep-alive pings: the socket is the app's alone, so the companion hears the app end when
    // it closes, and the crash handler's own ping below is the only one, so its answer is its own.
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

/// A client of the companion at `socket`, once it serves, within [`crate::CONNECT_FOR`].
///
/// On macOS the companion binds its socket and then checks in the Mach service a crash is sent
/// over (minidumper's `Server::with_name`), and `Client::with_name` connects the socket and then
/// looks the service up. An attempt between the two closed the socket it had opened, and the
/// companion, which ends when its app's socket closes, took that for the app's end: the retry
/// reached a companion on its way out, and the app's crash left no minidump (#6909). So no
/// attempt is made until the service is there, which is after the socket is bound.
fn connect(socket: &Path) -> Result<Client, minidumper::Error> {
    let until = std::time::Instant::now() + crate::CONNECT_FOR;
    loop {
        let attempt = serving(socket).and_then(|()| Client::with_name(SocketName::Path(socket)));
        match attempt {
            Ok(client) => return Ok(client),
            Err(_) if std::time::Instant::now() < until => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(why) => return Err(why),
        }
    }
}

/// Whether the companion at `socket` has checked in its Mach service, which it does once its
/// socket is bound: the look-up `Client::with_name` makes after connecting, made first.
#[cfg(target_os = "macos")]
fn serving(socket: &Path) -> Result<(), minidumper::Error> {
    let name = socket
        .to_str()
        .and_then(|name| std::ffi::CString::new(name).ok())
        .ok_or(minidumper::Error::InvalidPortName)?;
    crash_context::ipc::Client::create(&name)
        .map(drop)
        .map_err(minidumper::Error::from)
}

/// No Mach service elsewhere: the socket alone is the companion.
#[cfg(not(target_os = "macos"))]
fn serving(_socket: &Path) -> Result<(), minidumper::Error> {
    Ok(())
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
    // makes two calls on a client connected before the crash. Audited in minidumper 0.11 and
    // crash-context 0.8.0, neither takes a lock or allocates: `ping` is `send_message_impl` (one
    // `writev` of a stack header) and one `recv` into a stack buffer. On macOS, `request_dump` is
    // crash-context's `send_crash_context` (`mach_port_allocate` for the ack port and
    // `mach_msg`, no heap) and `AckReceiver`'s `mach_msg` into a stack message. On Linux it is
    // `as_bytes` of the context, one `writev` and one `recv`. It never panics: both errors are
    // values, and `Handled(false)` passes the crash on to the system's own handler.
    unsafe {
        crash_handler::make_crash_event(move |crash: &CrashContext| {
            // The companion answers a ping only once it has read every message sent before it on
            // this socket, and nothing else pings on it, so a `panicked` sent just before the
            // crash is heard before the dump is asked for.
            let pinged = client.ping();
            let dumped = client.request_dump(crash);
            eprintln!("[DEBUG-6909] on_crash ping={pinged:?} dump={dumped:?}");
            CrashEventResult::Handled(dumped.is_ok())
        })
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use std::os::unix::net::UnixListener;

    /// The companion binds its socket before it checks in its Mach service (minidumper's
    /// `Server::with_name`). An app that connected the socket in that gap gave up on the missing
    /// service and closed it, and the companion took the close for the app's end and exited: the
    /// app's retry then reached a companion on its way out, and its crash left no minidump (#6909).
    #[test]
    fn the_app_opens_no_socket_before_the_companions_mach_service_is_up() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("crash.sock");
        // A companion caught between the two: its socket bound, its Mach service not yet there.
        let bound = UnixListener::bind(&socket).unwrap();
        bound.set_nonblocking(true).unwrap();

        let connected = super::connect(&socket);

        assert!(connected.is_err(), "no companion served");
        let opened = bound.accept();
        assert!(
            opened
                .as_ref()
                .is_err_and(|why| why.kind() == std::io::ErrorKind::WouldBlock),
            "the app opened the socket of a companion not yet serving: {opened:?}"
        );
    }
}
