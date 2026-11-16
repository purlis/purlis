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
//!
//! **A host that is a login item** (FD-17, #655; ADR 0068 §2, X24) is started by the service
//! manager, not the window ([`serve_as_a_login_item_if_asked`]): only while something keeps it
//! running (a trigger or a paired device), it serves until it is told to quit, by the tray's
//! Stop or a quit with nothing keeping it, and a window that goes without a word leaves every
//! program running, with no grace period. Finding another host serving, it has nothing to do,
//! and exits as a success, so the manager does not start it again.

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
mod lateness;
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
#[cfg(unix)]
mod signingin;
#[cfg(all(unix, any(test, feature = "test-support")))]
pub mod standin;
#[cfg(unix)]
mod stopping;
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
pub use hearing::{Channel, HandOn, Hear, HearsNothing, TurnToUndo};
#[cfg(unix)]
pub use hosting::{
    ASK_WITHIN, AskTheWindow, Beside, Compose, ComposesNothing, ENDS_WITHIN, HeldToForget, Hosting,
    Looked, Older, Poke, ReachOf, Run, Running, Save, SavesNothing, Saving, Terminals, ToCompose,
    ToSave, Working,
};
#[cfg(unix)]
pub use hostrecord::{HOST_RECORD, READ_AT_MOST, Started, this_boot};
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
pub use routing::{DrainsTo, RelaysTo, Routed, RunsAgain};
#[cfg(unix)]
pub use serving::{
    Answering, DeletingOwnFolders, Delivering, Forgetting, Holding, HostError, Keeping, LOCK,
    Listing, Pairing, Place, SigningIn, Unpairing, Window, answers, serve, serve_answering,
};
#[cfg(unix)]
pub use stopping::{TOLD_TO_STOP, told_to_stop};
#[cfg(unix)]
pub use unattended::{Idles, UNLINKED_FOR};

/// The argument that starts the window's binary as its session host. Its word is the window's
/// own business: the binary is the window's, and only the window starts it so.
pub const SERVE_FOR_THE_WINDOW: &str = "purlis-serve-for-the-window";

/// The argument the service manager starts the window's binary with as a login item (FD-17).
pub const SERVE_AS_A_LOGIN_ITEM: &str = "purlis-serve-as-a-login-item";

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
            run(
                &place,
                Some(window),
                hosting,
                None,
                Serves::ForTheWindow,
                || {},
                async move {
                    lives
                        .while_a_window_may_come_back(GRACE, until_the_window_closes_its_end())
                        .await;
                },
            )
        }
        None => {
            say("there is no home folder to keep the session host's credentials in");
            1
        }
    })
}

/// When the window's binary was started by the service manager as the session host's login
/// item ([`SERVE_AS_A_LOGIN_ITEM`]), serve at the place `place` names until a window or the
/// tray tells it to quit ([`Hosting::while_it_is_a_login_item`]), holding its programs and
/// projects in what `hosting` gives, and give the code to exit with; otherwise `None`, before
/// anything is read or opened.
///
/// No window started it, so no window is its parent and its programs run below it alone, as
/// `purlis serve`'s do. **Another host serving is a success** ([`exit_code`]): a window started
/// one before the manager did, and the manager is not to start this one again.
///
/// `serving` is called once this host serves, after it has found no other host serving and
/// taken the lock: what only the host that serves may do (#6687, keeping its log capped), and
/// never a second host about to exit.
#[cfg(unix)]
pub fn serve_as_a_login_item_if_asked(
    place: impl FnOnce() -> Option<Place>,
    hosting: impl FnOnce(&Place) -> Hosting,
    serving: impl FnOnce(),
) -> Option<i32> {
    if std::env::args_os().nth(1)? != SERVE_AS_A_LOGIN_ITEM {
        return None;
    }
    Some(match place() {
        Some(place) => {
            let hosting = hosting(&place);
            let lives = hosting.clone();
            run(
                &place,
                None,
                hosting,
                None,
                Serves::AsALoginItem,
                serving,
                async move {
                    lives.while_it_is_a_login_item().await;
                },
            )
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
    run(
        place,
        None,
        hosting(place),
        None,
        Serves::Alone,
        || {},
        std::future::pending(),
    )
}

/// [`serve_alone`], as a runner's host: it also answers links from the one desktop it is paired
/// with ([`Pairing`]) on its link socket ([`Place::link`]), which the bridge a connector runs
/// opens (ADR 0078 §2). **Paired, it suspends itself after `suspends` with no link from its
/// desktop** (#2051, [`UNLINKED_FOR`] by default), where it is given one: it stops serving, and
/// every program it runs ends as at a quit. **Paired, it also stops once it has been idle**
/// ([`Idles`], #3980), where it is told to. `serves` says who started it: a service manager
/// ([`Serves::AsARunnersService`], RR-15) takes another host already serving as a success, and
/// any `purlis serve` hears `SIGTERM` as a quit (#6700).
#[cfg(unix)]
pub fn serve_alone_answering(
    place: &Place,
    hosting: impl FnOnce(&Place) -> Hosting,
    pairing: Pairing,
    suspends: Option<std::time::Duration>,
    idles: Option<Idles>,
    serves: Serves,
) -> i32 {
    let hosting = hosting(place);
    let answering = std::sync::Arc::clone(&pairing.answering);
    let answering_too = std::sync::Arc::clone(&answering);
    let unlinked = {
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
    let idle = {
        let hosting = hosting.clone();
        async move {
            match idles {
                Some(Idles { after, said }) => {
                    hosting
                        .until_idle_for(after, move || matches!(answering_too(), Ok(Some(_))))
                        .await;
                    said();
                }
                None => std::future::pending().await,
            }
        }
    };
    let suspended = async move {
        tokio::select! {
            () = unlinked => {}
            () = idle => {}
        }
    };
    run(
        place,
        None,
        hosting,
        Some(pairing),
        serves,
        || {},
        suspended,
    )
}

#[cfg(unix)]
/// Who a host serves for, which decides what finding another serving means ([`exit_code`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Serves {
    /// The window started it, and reattaches to the one serving (exit code 3).
    ForTheWindow,
    /// The service manager started it as the desktop's login item (FD-17): another serving is
    /// a success.
    AsALoginItem,
    /// The service manager started it as a runner's host, `purlis serve --as-a-service`
    /// (RR-15): another serving is a success, as for a login item, and `SIGTERM` is a quit.
    AsARunnersService,
    /// `purlis serve`, on a runner or at a terminal.
    Alone,
}

#[cfg(unix)]
impl Serves {
    /// **Whether `SIGTERM` is heard as a quit** (#6700, ADR 0068 §3 as amended for #6700):
    /// `purlis serve`, which systemd or launchd stops by it at a shutdown, a stop or a removal,
    /// ends every program as a quit ([`told_to_stop`]) and empties its host record. The window's
    /// own host and the desktop's login item keep the signal's default, an end as sudden as a
    /// crash's, whose programs the next host settles (#2007).
    pub fn hears_sigterm_as_a_quit(self) -> bool {
        matches!(self, Serves::AsARunnersService | Serves::Alone)
    }
}

#[cfg(unix)]
/// **The code a host exits with**, by how serving went and who it serves for: 0 when it stopped
/// as asked, 3 when another host of this user already serves (which the window reattaches to),
/// and 1 for any other failure. A login item finding another serving exits 0: the service
/// manager restarts it only after a failure, and a host already serving is none.
pub fn exit_code(served: Result<(), &HostError>, serves: Serves) -> i32 {
    match (served, serves) {
        (Ok(()), _)
        | (
            Err(HostError::AlreadyServing { .. }),
            Serves::AsALoginItem | Serves::AsARunnersService,
        ) => 0,
        (Err(HostError::AlreadyServing { .. }), _) => 3,
        (Err(_), _) => 1,
    }
}

#[cfg(unix)]
fn run(
    place: &Place,
    window: Option<Window>,
    hosting: Hosting,
    pairing: Option<Pairing>,
    serves: Serves,
    serving: impl FnOnce(),
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
    // Said in this host's log whenever its one thread was held, and, where asked, sampled while
    // it is (#6437).
    let beat = lateness::Beat::new();
    runtime.spawn(lateness::watch_the_thread(beat.clone()));
    lateness::sample_when_held(beat);
    let ready = || {
        use std::io::Write;
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{READY}").and_then(|()| out.flush());
        serving();
    };
    let served = runtime.block_on(async {
        // In before the host says it is ready, so a stop sent the moment it serves is heard.
        let told = if serves.hears_sigterm_as_a_quit() {
            match told_to_stop() {
                Ok(told) => Some(told),
                Err(why) => {
                    say(&format!(
                        "SIGTERM and SIGINT could not be handled, so a stop ends this host as \
                         a crash would: {why}"
                    ));
                    None
                }
            }
        } else {
            None
        };
        let until = async move {
            match told {
                Some(told) => tokio::select! {
                    () = until => {}
                    () = told => {}
                },
                None => until.await,
            }
        };
        serve_answering(place, window, hosting, pairing, ready, until).await
    });
    if let Err(why) = &served {
        say(&why.to_string());
    }
    exit_code(served.as_ref().map(|_| ()), serves)
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn another_serves() -> HostError {
        HostError::AlreadyServing {
            socket: "/the/socket".into(),
        }
    }

    #[test]
    fn a_login_item_finding_another_host_serving_exits_as_a_success() {
        assert_eq!(
            exit_code(Err(&another_serves()), Serves::AsALoginItem),
            0,
            "the manager restarts it only after a failure, and another serving is none"
        );
    }

    #[test]
    fn the_windows_host_finding_another_serving_says_so_for_the_window_to_reattach() {
        assert_eq!(exit_code(Err(&another_serves()), Serves::ForTheWindow), 3);
        assert_eq!(exit_code(Err(&another_serves()), Serves::Alone), 3);
        assert_eq!(
            exit_code(Err(&another_serves()), Serves::AsARunnersService),
            0,
            "a runner's service finding its version's host serving is a success too"
        );
    }

    /// #6700: systemd and launchd stop `purlis serve` by `SIGTERM`, which it hears as a quit.
    /// The window's own host and the desktop's login item are unchanged.
    #[test]
    fn only_purlis_serve_hears_sigterm_as_a_quit() {
        assert!(Serves::Alone.hears_sigterm_as_a_quit());
        assert!(Serves::AsARunnersService.hears_sigterm_as_a_quit());
        assert!(!Serves::ForTheWindow.hears_sigterm_as_a_quit());
        assert!(!Serves::AsALoginItem.hears_sigterm_as_a_quit());
    }

    /// #6687: what only the host that serves may do (keep its log trimmed) is never done by a
    /// login item that finds another host serving and is about to exit. The other host is its
    /// lock held, which a second host finds before it touches the socket.
    #[test]
    fn a_login_item_finding_another_host_serving_never_starts_what_only_the_serving_host_does() {
        let dir = tempfile::tempdir().unwrap();
        let place = Place::at(
            dir.path().join("purlis-test").join("purlisd.sock"),
            dir.path().join("config").join("purlisd"),
        );
        std::fs::create_dir_all(place.socket.parent().unwrap()).unwrap();
        let held = std::fs::File::create(&place.lock).unwrap();
        held.try_lock().unwrap();
        let started = std::cell::Cell::new(false);

        let code = run(
            &place,
            None,
            Hosting::new(ComposesNothing),
            None,
            Serves::AsALoginItem,
            || started.set(true),
            std::future::pending(),
        );

        assert_eq!(code, 0, "another serving is a success for a login item");
        assert!(
            !started.get(),
            "the second host started what the serving one does"
        );
    }

    #[test]
    fn a_host_that_stopped_as_asked_exits_0_whoever_it_serves_for() {
        for serves in [
            Serves::ForTheWindow,
            Serves::AsALoginItem,
            Serves::AsARunnersService,
            Serves::Alone,
        ] {
            assert_eq!(exit_code(Ok(()), serves), 0, "{serves:?}");
        }
    }
}
