// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The session host, `purlisd` (ADR 0068): one per OS user per device, serving every project
//! that user opens. On a desktop the window's own binary is started again as its host, so the
//! program and its Keychain identity are the window's and nothing prompts (ADR 0068 §1, amended
//! 2026-10-14); on a runner it is `purlis serve`. Both run [`serve`].
//!
//! **What it does** (#1997, #1998): it listens on its socket, admits the scopes whose
//! credentials it minted, refuses everyone else, and serves the session protocol: it holds
//! projects, and starts, views, drives, stops and closes the programs a client starts in them
//! ([`Hosting`]). How a start is composed is the binary's ([`Compose`]), and so is how a chat's
//! confinement, its egress proxy, is started from the window's plan (#1999, [`Confine`]), each
//! built on the core, so this crate links none of it.
//!
//! **How it starts, in order, each step failing closed:**
//!
//! 1. The socket's folder is made this user's alone (`0700`, owned, never a link:
//!    `purlis_same_user::PrivateDirectory`).
//! 2. The host takes the folder's lock. A second host of the same user finds it held and exits
//!    with a sentence saying so ([`HostError::AlreadyServing`]), before it touches the socket or
//!    the credentials of the one that is serving.
//! 3. A socket file left by a host that ended without removing it is removed, which only the
//!    lock's holder may do, and the socket is bound and made `0600`.
//! 4. Every scope's credential is minted fresh into the machine store's folder
//!    (`purlis_session_protocol::auth::Credentials::mint_into`), so a credential lives as long
//!    as the host that minted it.
//! 5. It says it is ready, and accepts.
//!
//! **Who is admitted** is the protocol crate's checks, the same the hook sockets make: another
//! uid is closed unread (`local::Listener`); a peer inside a chat is refused whatever it proves
//! (`link::serve` and `local::Chats`), and while chats run in the window, a peer below the
//! window is one ([`Window`]); a peer that cannot prove a scope's credential is refused
//! (`auth`). The host adds no check of its own and skips none.
//!
//! **When it ends.** The window's host ends when the window does: the window holds the other
//! end of its standard input, and the host stops when that closes, whether the window quit,
//! crashed or was killed ([`serve_for_the_window_if_asked`]). On the way out it removes its
//! socket.

#[cfg(unix)]
mod browsers;
#[cfg(unix)]
mod confining;
#[cfg(unix)]
mod hearing;
#[cfg(unix)]
mod hosting;
#[cfg(unix)]
mod killswitch;
#[cfg(unix)]
mod relay;
#[cfg(unix)]
mod serving;
#[cfg(all(unix, any(test, feature = "test-support")))]
pub mod standin;

#[cfg(unix)]
pub use browsers::{Browsers, FrameSink, NoBrowsers, Screen, THUMBNAIL_EVERY};
#[cfg(unix)]
pub use confining::{Askable, Confine, ConfinesNothing, Confining, Tell, ToConfine};
#[cfg(unix)]
pub use hearing::{Channel, HandOn, Hear, HearsNothing};
#[cfg(unix)]
pub use hosting::{
    ASK_WITHIN, AskTheWindow, Compose, ComposesNothing, ENDS_WITHIN, FromTheWindow, Hosting,
    Looked, Poke, Run, Running, Save, SavesNothing, Saving, Terminals, ToCompose, ToSave, Working,
};
#[cfg(unix)]
pub use killswitch::KILL_SWITCH;
/// The session protocol this host speaks, for a binary's [`Compose`].
#[cfg(unix)]
pub use purlis_session_protocol as protocol;
#[cfg(unix)]
pub use relay::{Dial, Relay};
#[cfg(unix)]
pub use serving::{HostError, LOCK, Place, Window, serve};

/// The argument that starts the window's binary as its session host. Its word is the window's
/// own business: the binary is the window's, and only the window starts it so.
pub const SERVE_FOR_THE_WINDOW: &str = "purlis-serve-for-the-window";

/// The line the host writes on its standard output once it accepts on its socket.
pub const READY: &str = "ready";

/// When the window's binary was started as its session host ([`SERVE_FOR_THE_WINDOW`]), serve
/// at the place `place` names until the window closes this process's standard input, holding
/// its programs and projects in what `hosting` gives (which composes each start, hears each held
/// project's hooks, starts each chat's confinement and starts its browsers), and give the code
/// to exit with; otherwise `None`, before anything is read or opened. `hosting` is given the
/// place, whose credentials folder is where the host keeps its kill switch ([`Hosting::kept`]).
///
/// The window is this process's parent, and every chat it runs is below it, so a peer below it
/// is inside a chat ([`Window`]). The ready line goes to standard output and what the host has
/// to say to standard error, which the window keeps in its log.
#[cfg(unix)]
pub fn serve_for_the_window_if_asked(
    place: impl FnOnce() -> Option<Place>,
    hosting: impl FnOnce(&Place) -> Hosting,
) -> Option<i32> {
    if std::env::args_os().nth(1)? != SERVE_FOR_THE_WINDOW {
        return None;
    }
    let window = Window(std::os::unix::process::parent_id());
    Some(match place() {
        Some(place) => run(
            &place,
            Some(window),
            // The kill switch, as this host's last one left it, read before anyone is served
            // (ADR 0071): the window's binary keeps it in the place's credentials folder.
            hosting(&place),
            until_the_window_closes_its_end(),
        ),
        None => {
            say("there is no home folder to keep the session host's credentials in");
            1
        }
    })
}

/// Serve at `place` until this process is ended, as `purlis serve` does, holding the programs
/// `hosting` makes: no window started it, so its programs run below the host alone. The exit
/// code is what a failure to start gives.
#[cfg(unix)]
pub fn serve_alone(place: &Place, hosting: impl FnOnce(&Place) -> Hosting) -> i32 {
    run(place, None, hosting(place), std::future::pending())
}

#[cfg(unix)]
fn run(
    place: &Place,
    window: Option<Window>,
    hosting: Hosting,
    until: impl Future<Output = ()>,
) -> i32 {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(why) => {
            say(&format!(
                "the session host could not start its runtime: {why}"
            ));
            return 1;
        }
    };
    let ready = || {
        use std::io::Write;
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{READY}").and_then(|()| out.flush());
    };
    match runtime.block_on(serve(place, window, hosting, ready, until)) {
        Ok(()) => 0,
        Err(why) => {
            say(&why.to_string());
            match why {
                HostError::AlreadyServing { .. } => 3,
                _ => 1,
            }
        }
    }
}

/// Resolves once this process's standard input reaches its end, or fails: the window holds the
/// other end, and the kernel closes it however the window goes.
#[cfg(unix)]
async fn until_the_window_closes_its_end() {
    use tokio::io::AsyncReadExt;
    let mut stdin = tokio::io::stdin();
    let mut buffer = [0u8; 64];
    while matches!(stdin.read(&mut buffer).await, Ok(read) if read > 0) {}
}

/// One line on standard error, where the host's log is.
pub(crate) fn say(line: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "purlisd: {line}");
}
