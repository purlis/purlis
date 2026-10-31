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
//! ([`Hosting`]). While chats run in it, it is the event log's one writer, and keeps what each
//! program's end leaves, window or no window (#2003, [`Record`]). How a start is composed is the
//! binary's ([`Compose`]), and so is how a chat's confinement, its egress proxy, is started from
//! the window's plan (#1999, [`Confine`]), each built on the core, so this crate links none of it.
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
//! **When it ends** (ADR 0068 §2, #2006). The window's host ends when the window quits
//! (`quit`), ending every program as a quit. A window that goes without a word crashed or was
//! killed: the host keeps its programs for the grace period ([`GRACE`]), so a window that starts
//! within it takes them back, and ends them as a quit would once it passes. The window holds the
//! other end of its standard input, which says when the window that started it has gone
//! ([`serve_for_the_window_if_asked`]). On the way out it removes its socket.

#[cfg(unix)]
mod browsers;
#[cfg(unix)]
mod code;
#[cfg(unix)]
mod confining;
#[cfg(unix)]
mod connector;
#[cfg(unix)]
mod grading;
#[cfg(unix)]
mod hearing;
mod hosting;
#[cfg(unix)]
mod hostrecord;
#[cfg(unix)]
mod killswitch;
#[cfg(unix)]
mod lifetime;
#[cfg(unix)]
mod ownlog;
#[cfg(unix)]
mod reach;
#[cfg(unix)]
mod recording;
#[cfg(unix)]
mod relay;
#[cfg(unix)]
mod routing;
#[cfg(unix)]
mod serving;
#[cfg(all(unix, any(test, feature = "test-support")))]
pub mod standin;
#[cfg(unix)]
mod unattended;

pub use browsers::NotStarted as BrowserNotStarted;
#[cfg(unix)]
pub use browsers::{
    A_BROWSER_THE_PERSON_DRIVES, AN_ENDED_BROWSER, Browsers, Download, ENGINE_DOWNLOAD_CANCELLED,
    FrameSink, INPUT_MISSED, LINK_MOST_FRAME_WIDTH, LINK_MOST_FRAMES_A_SECOND, LoginEnded,
    NOT_DRIVING, NoBrowsers, NotActed, NotTaken, Screen, ScreenEnded, THUMBNAIL_EVERY,
};
#[cfg(unix)]
pub use code::{Carried, Code, MovesNoCode, TRANSFER_WITHIN};
#[cfg(unix)]
pub use confining::{Askable, Confine, ConfinesNothing, Confining, Planned, Tell, ToConfine};
#[cfg(unix)]
pub use connector::{
    BeforeEach, BeforeEachChecked, CommandConnector, Connected, NotChecked, Prepare, Prepared,
};
#[cfg(unix)]
pub use hearing::{Channel, HandOn, Hear, HearsNothing};
#[cfg(unix)]
pub use hosting::{
    ASK_WITHIN, AskTheWindow, Beside, Compose, ComposesNothing, ENDS_WITHIN, Hosting, Looked,
    OwnFoldersHeld, Poke, ReachOf, Run, Running, Save, SavesNothing, Saving, Terminals, ToCompose,
    ToSave, Working,
};
#[cfg(unix)]
pub use hostrecord::{HOST_RECORD, Started, this_boot};
#[cfg(unix)]
pub use killswitch::{KILL_SWITCH, NoStopsElsewhere, StopsElsewhere};
#[cfg(unix)]
pub use lifetime::GRACE;
/// The session protocol this host speaks, for a binary's [`Compose`].
#[cfg(unix)]
pub use purlis_session_protocol as protocol;
#[cfg(unix)]
pub use reach::{EveryRunner, REACHED_WITHIN, TRIED_AGAIN_EVERY};
#[cfg(unix)]
pub use recording::{Ended, ReadsLog, Record, RecordsNothing, Stopped};
#[cfg(unix)]
pub use relay::{Dial, Keep, Relay};
#[cfg(unix)]
pub use routing::{DrainsTo, RelaysTo, Routed};
#[cfg(unix)]
pub use serving::{
    Answering, DeletingOwnFolders, Delivering, Forgetting, Holding, HostError, Keeping, LOCK,
    Listing, Pairing, Place, Unpairing, Window, serve, serve_answering,
};
#[cfg(unix)]
pub use unattended::UNLINKED_FOR;

/// The argument that starts the window's binary as its session host. Its word is the window's
/// own business: the binary is the window's, and only the window starts it so.
pub const SERVE_FOR_THE_WINDOW: &str = "purlis-serve-for-the-window";

/// The line the host writes on its standard output once it accepts on its socket.
pub const READY: &str = "ready";

/// When the window's binary was started as its session host ([`SERVE_FOR_THE_WINDOW`]), serve
/// at the place `place` names for as long as a window may come back to it (`crate::lifetime`:
/// until the window quits, or the grace period passes with no window and nothing to wait for;
/// the window that started it holds this process's standard input), holding
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
        Some(place) => {
            // The kill switch, as this host's last one left it, read before anyone is served
            // (ADR 0071): the window's binary keeps it in the place's credentials folder.
            let hosting = hosting(&place);
            let lives = hosting.clone();
            run(&place, Some(window), hosting, None, async move {
                lives
                    .while_a_window_may_come_back(GRACE, until_the_window_closes_its_end())
                    .await;
            })
        }
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
    run(place, None, hosting(place), None, std::future::pending())
}

/// [`serve_alone`], as a runner's host: it also answers links from the one desktop it is paired
/// with ([`Pairing`]) on its link socket ([`Place::link`]), which the bridge a connector runs
/// opens (ADR 0078 §2). **Paired, it suspends itself after `suspends` with no link from its
/// desktop** (#2051, [`UNLINKED_FOR`] by default), where it is given one: it stops serving, and
/// every program it runs ends as at a quit.
#[cfg(unix)]
pub fn serve_alone_answering(
    place: &Place,
    hosting: impl FnOnce(&Place) -> Hosting,
    pairing: Pairing,
    suspends: Option<std::time::Duration>,
) -> i32 {
    let hosting = hosting(place);
    let answering = std::sync::Arc::clone(&pairing.answering);
    let suspended = {
        let hosting = hosting.clone();
        async move {
            match suspends {
                Some(after) => {
                    hosting
                        .until_unlinked_for(after, move || matches!(answering(), Ok(Some(_))))
                        .await;
                }
                None => std::future::pending().await,
            }
        }
    };
    run(place, None, hosting, Some(pairing), suspended)
}

#[cfg(unix)]
fn run(
    place: &Place,
    window: Option<Window>,
    hosting: Hosting,
    pairing: Option<Pairing>,
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
    match runtime.block_on(serve_answering(
        place, window, hosting, pairing, ready, until,
    )) {
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

/// Resolves once this process's standard input reaches its end, or fails: the window that
/// started this host holds the other end, and the kernel closes it however the window goes.
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
