// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How an ask finds the app, and knows it is the app** (#6995).
//!
//! An ask is a question whose answer decides something: whether a chat is attended, whether a
//! merge is held, whether a branch folder is another chat's, whether the app runs a chat's git
//! for it. Every process a chat starts inherits the chat's environment and working directory
//! and may change both, so neither says where the app is, nor whether it is the app, nor which
//! chat is asking. The process tree and the sandbox do:
//!
//! - **Where this process stands** ([`Under`]), read from the kernel: its ancestors, each with
//!   the program the kernel mapped for it.
//!   - [`Under::App`]: the whole chain is read, and the app runs above this process. The app is
//!     the topmost ancestor running the app's program (`purlis-app`, or a `purlis` that holds a
//!     plane's hook socket, as `purlis serve` does: [`program_of`]), with the ancestors that run
//!     purlis directly below it (the window and its session host). What a program is to an ask
//!     is read from that program alone, never from where this process's own program is, so a
//!     copy of `purlis` in a chat's folder neither vouches for a program beside it nor makes the
//!     real app untrusted. **The listener must be one of those very processes, and the socket
//!     is one it holds** ([`purlis_same_user::admit_host`]): neither the environment nor the
//!     working directory chooses it. A listener a chat starts, even a real `purlis serve` in a
//!     plane of its own making or a copy of the app's program, runs below the chat's harness,
//!     which is not the app's program, so it is never one of them. **Under the app, only the
//!     app's positive answer that this process is in none of its chats ([`IN_NO_CHAT`]) lets a
//!     command run here**: an app that is slow, older than the question, full, or holds no hook
//!     socket leaves it refused.
//!   - [`Under::NoApp`]: the whole chain is read, and no app runs above. This is no chat's:
//!     the person's own terminal, where the command runs here.
//!   - [`Under::Boxed`]: an ancestor, or its program, cannot be read, or this process runs in a
//!     pid namespace of its own, whose first process reads as the top of the chain. That is a
//!     sandbox's boundary (Seatbelt shows a process none it did not start; Landlock refuses the
//!     program of one outside it; `unshare -p` hides every process above), and a sandboxed
//!     process is always a chat's. **The socket must be in a folder a sandbox's rule keeps this
//!     process from writing** ([`may_not_write`]): one this user owns and may write by its
//!     permissions, which no flag or access list closes, and which is refused all the same.
//!     Nothing the sandbox runs can have bound it, and nothing it runs can lift that rule, as it
//!     can a folder's mode or flags. The
//!     socket named in `$PURLIS_HOOK_SOCKET`, then the plane's above the working directory, are
//!     where it knocks; a socket planted anywhere this process may write is refused.
//! - **Which chat is asking is the app's to say** ([`WhoAmI`]): it answers from the asker's
//!   ancestry and its own record of each chat's program, and hands back that chat's number and
//!   token. The environment's chat is put to an app older than the question, which judges it by
//!   the same ancestry as every line.
//! - **The environment may only tighten.** Where it names a chat, or this process is boxed, and
//!   no app answers, every ask is answered strictly ([`Unreached::AppGone`]), never run here.
//!
//! **What this cannot prove**, in [`Under`]: a process that leaves the chat's tree on purpose
//! (a double fork to `launchd` or `init`) and empties its environment reads as the person's
//! terminal to itself. It runs unsandboxed then, with the person's own rights, and the app still
//! refuses every line it sends (it is inside no chat).

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{Answer, CHAT_ENV, ChatToken};

/// What the app is told the asker wants to know: which chat it runs in. `{"whoami": true}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WhoAmI {
    pub whoami: bool,
}

/// The app's answer to [`WhoAmI`]: the chat the asker runs inside, by its ancestry, and that
/// chat's token, which every line it writes then carries.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct YouAre {
    pub you_are: u32,
    pub token: String,
}

impl std::fmt::Debug for YouAre {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("YouAre")
            .field("you_are", &self.you_are)
            .finish_non_exhaustive()
    }
}

/// What the app answers a [`WhoAmI`] from a process that runs inside none of its chats.
pub const IN_NO_CHAT: &str = "this process runs inside none of this app's chats";

/// How an ask that found no app says so, and every sentence that refuses one starts with it.
pub const NO_APP_ANSWERED: &str = "no purlis app answered for this chat";

/// The name of the socket the app binds for a plane's hooks ([`crate::hookchannel::socket_for`]).
const HOOK_SOCKET: &str = "hooks.sock";

/// The app, reached and authenticated, and the chat it says this process runs in.
#[derive(Debug)]
pub struct Reached {
    /// The chat's number, as the app named it, or as the environment claims it to an app older
    /// than [`WhoAmI`].
    pub chat: u32,
    /// The chat's token, from the app, or from the environment where the app is older.
    pub token: Option<ChatToken>,
    /// The connection, authenticated, ready for the ask.
    pub stream: UnixStream,
    /// The socket it is connected to.
    pub socket: PathBuf,
}

/// Why no app can be asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unreached {
    /// This process is no chat's: no app runs above it and nothing says it is a chat's, or the
    /// app above it says it runs in none of its chats. The command runs here, as it does in a
    /// person's terminal.
    NotAChat,
    /// This process is a chat's (its environment names a chat, or a sandbox holds it) and no
    /// app answers: answered strictly, never run here.
    AppGone(String),
    /// Something answers that is not the app: answered strictly, never run here.
    NotTheApp(String),
}

impl Unreached {
    /// The sentence an ask refused for this says, where it is refused.
    pub fn said(&self) -> Option<String> {
        match self {
            Self::NotAChat => None,
            Self::AppGone(why) | Self::NotTheApp(why) => Some(format!("{NO_APP_ANSWERED}: {why}")),
        }
    }
}

/// Where this process stands, as the kernel says: what an ask believes about the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Under {
    /// The app runs above this process: the topmost ancestor running the app's program, and
    /// those directly below it in this process's chain that run it too, topmost first.
    App(Vec<u32>),
    /// Every ancestor was read, and none runs the app's program.
    NoApp,
    /// An ancestor, or the program of one, could not be read: a sandbox's boundary.
    Boxed,
}

/// Where this process stands now ([`Under`]).
pub fn under() -> Under {
    use purlis_same_user::Parents;
    let me = std::process::id();
    let chain = Parents::chain(me);
    // The chain ends below pid 1 only where a parent could not be read; a parent outside this
    // process's pid namespace reads as 0, so the first process of a namespace of its own reads
    // as the top too, and only the system's own namespace has its top at the system's start.
    let top = chain.last().copied().unwrap_or(me);
    if !matches!(Parents::of(top), Some(0 | 1)) || !in_the_system_s_pid_namespace() {
        return Under::Boxed;
    }
    let programs = purlis_same_user::executables_of(&chain, crate::forklock::output);
    let Some(programs) = programs.into_iter().collect::<Option<Vec<PathBuf>>>() else {
        return Under::Boxed;
    };
    let mut ran: Vec<Ran> = programs.iter().map(|program| program_of(program)).collect();
    // A `purlis` above the topmost app is the app only where it holds a plane's hook socket, as
    // `purlis serve` does; any other is a command the person ran, never an app.
    let above = ran
        .iter()
        .rposition(|it| *it == Ran::App)
        .map_or(0, |at| at + 1);
    let purlises: Vec<usize> = (above..ran.len())
        .filter(|at| ran[*at] == Ran::Purlis)
        .collect();
    if !purlises.is_empty() {
        let pids: Vec<u32> = purlises.iter().map(|at| chain[*at]).collect();
        let held = purlis_same_user::sockets_held_by(&pids, crate::forklock::output);
        for (at, sockets) in purlises.into_iter().zip(held) {
            if sockets
                .iter()
                .any(|path| path.file_name().is_some_and(|name| name == HOOK_SOCKET))
            {
                ran[at] = Ran::Serves;
            }
        }
    }
    under_of(&chain, &ran)
}

/// What an ancestor's program is to an ask (#6995), read from the program alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ran {
    /// The app's own program, `purlis-app`.
    App,
    /// `purlis`, holding a plane's hook socket: `purlis serve`, the app without a window.
    Serves,
    /// `purlis`, holding none: a command, or the app's session host directly below it.
    Purlis,
    /// Anything else.
    Other,
}

/// [`under`]'s decision, given this process's chain, parent first, and what each runs: the
/// topmost that is the app ([`Ran::App`] or [`Ran::Serves`]), and the run of purlis's programs
/// directly below it.
pub fn under_of(chain: &[u32], ran: &[Ran]) -> Under {
    let Some(topmost) = ran
        .iter()
        .rposition(|it| matches!(it, Ran::App | Ran::Serves))
    else {
        return Under::NoApp;
    };
    let mut app = vec![chain[topmost]];
    for at in (0..topmost).rev() {
        if ran[at] == Ran::Other {
            break;
        }
        app.push(chain[at]);
    }
    Under::App(app)
}

/// Whether this process runs in the system's own pid namespace, where its chain can reach the
/// system's first process. Elsewhere than Linux there is only the one.
fn in_the_system_s_pid_namespace() -> bool {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        std::fs::read_link("/proc/self/ns/pid")
            .ok()
            .is_some_and(|link| the_system_s_pid_namespace(&link))
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        true
    }
}

/// Whether `link`, what `/proc/self/ns/pid` reads, names the system's own pid namespace: the
/// kernel gives it the one fixed number, `PROC_PID_INIT_INO`, and every other a fresh one.
pub fn the_system_s_pid_namespace(link: &Path) -> bool {
    link == Path::new("pid:[4026531836]")
}

/// The socket the app binds for the plane above `cwd`, found by the plane's marker
/// ([`crate::plane::find_root`]): only where a sandbox keeps this process from reading the app,
/// as somewhere to knock, and believed only in a folder this process may not write.
pub fn socket_here(cwd: &Path) -> PathBuf {
    let plane = crate::plane::find_root(cwd).ok();
    crate::hookchannel::socket_for(plane.as_deref()).socket
}

/// The chat this process's environment says it runs in: only ever a reason to be stricter.
pub fn claimed_chat(env: &dyn Fn(&str) -> Option<String>) -> Option<u32> {
    [CHAT_ENV, crate::active::SESSION_ID_ENV]
        .into_iter()
        .filter_map(env)
        .find_map(|n| n.trim().parse::<u32>().ok().filter(|n| *n > 0))
}

/// The app for this process, standing in `cwd` with environment `env`, and the chat the app
/// says it runs in, each line waiting at most `within`; or why none can be asked.
pub fn reach(
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
    within: Duration,
) -> Result<Reached, Unreached> {
    reach_from(under(), cwd, env, within)
}

/// [`reach`], from where this process stands, `under`.
pub fn reach_from(
    under: Under,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
    within: Duration,
) -> Result<Reached, Unreached> {
    let claimed = claimed_chat(env);
    let (knock, admitted): (Vec<PathBuf>, Admit) = match under {
        Under::NoApp => {
            return Err(match claimed {
                Some(_) => Unreached::AppGone(
                    "this process does not run under the app that started its chat".to_owned(),
                ),
                None => Unreached::NotAChat,
            });
        }
        Under::App(app) => {
            let held = purlis_same_user::sockets_held_by(&app, crate::forklock::output);
            let mut sockets: Vec<PathBuf> = held
                .into_iter()
                .flatten()
                .filter(|path| path.file_name().is_some_and(|name| name == HOOK_SOCKET))
                .collect();
            sockets.dedup();
            // The one the environment names, alone, where the app holds it: the socket the app
            // handed this chat, so no other plane's is asked. Where it names none, every socket
            // the app holds is asked, so a socket made up never spares a chat the app's
            // question. Whichever is asked, only the app's own process above this one is
            // believed there.
            if let Some(hinted) = env(super::SOCKET_ENV).filter(|it| !it.is_empty()) {
                let hinted = PathBuf::from(hinted);
                if sockets.iter().any(|socket| same_socket(socket, &hinted)) {
                    sockets.retain(|socket| same_socket(socket, &hinted));
                } else if claimed.is_some() {
                    // Not one the app is seen to hold: asked all the same, and believed only
                    // where what listens there is the app's own process, above this one.
                    sockets = vec![hinted];
                }
            }
            (sockets, Admit::Ancestors(app))
        }
        Under::Boxed => {
            let hinted = env(super::SOCKET_ENV)
                .filter(|it| !it.is_empty())
                .map(PathBuf::from);
            let mut sockets: Vec<PathBuf> = hinted.into_iter().collect();
            let plane = socket_here(cwd);
            if !sockets.iter().any(|it| same_socket(it, &plane)) {
                sockets.push(plane);
            }
            (sockets, Admit::Unwritable)
        }
    };
    let mut said = Said::default();
    for socket in knock {
        match asked_at(&socket, &admitted, within) {
            Ok(Asked::Named(reached)) => return Ok(reached),
            Ok(Asked::InNoChat) => {
                said.in_no_chat.get_or_insert(socket);
            }
            // An app older than the question, or one that did not answer it in time: with the
            // connection, where it answered something else and kept it.
            Ok(Asked::Older(kept)) => {
                if said.older.as_ref().is_none_or(|(_, kept)| kept.is_none()) {
                    said.older = Some((socket, kept));
                }
            }
            Err(Unreached::AppGone(why)) => said.unheard = Some(why),
            Err(other) => return Err(other),
        }
    }
    let Some(chat) = claimed else {
        return Err(unclaimed(&admitted, said));
    };
    // The app says this process runs in none of its chats, or is older than the question: the
    // environment's chat is put to it, and the app judges it as it judges every line, by the
    // chat's token and this process's ancestry.
    let Some((socket, kept)) = said
        .older
        .or_else(|| said.in_no_chat.map(|socket| (socket, None)))
    else {
        return Err(Unreached::AppGone(said.unheard.unwrap_or_else(|| {
            "the app that started this chat holds no hook socket".to_owned()
        })));
    };
    let stream = match kept {
        Some(stream) => Ok(stream),
        None => connect_app(&socket, &admitted),
    };
    match stream {
        Ok(stream) => Ok(Reached {
            chat,
            token: ChatToken::read(env),
            stream,
            socket,
        }),
        Err(why) if why.kind() == io::ErrorKind::PermissionDenied => {
            Err(Unreached::NotTheApp(why.to_string()))
        }
        Err(why) => Err(Unreached::AppGone(format!(
            "the app at {} went away ({why})",
            socket.display()
        ))),
    }
}

/// What the sockets asked said, where none named the chat.
#[derive(Default)]
struct Said {
    /// The first whose app said this process runs in none of its chats.
    in_no_chat: Option<PathBuf>,
    /// One whose app is older than the question, or did not answer it in time.
    older: Option<(PathBuf, Option<UnixStream>)>,
    /// Why the last that did not answer did not.
    unheard: Option<String>,
}

/// Why no app can be asked for a process whose environment names no chat, given what the
/// sockets asked said.
///
/// **Only the app's own answer that this process runs in none of its chats lets the command run
/// here** (#6995): under the app ([`Admit::Ancestors`]) every socket asked must have said so. An
/// app that is slow, full, older than the question, or holds no hook socket says nothing about
/// this process, which runs under it all the same, so it is refused. A sandboxed process
/// ([`Admit::Unwritable`]) is a chat's whatever anything says.
fn unclaimed(admitted: &Admit, said: Said) -> Unreached {
    let socket = said
        .in_no_chat
        .as_deref()
        .or(said.older.as_ref().map(|(it, _)| it.as_path()));
    match (
        admitted,
        said.in_no_chat.is_some(),
        said.older.is_some(),
        said.unheard,
    ) {
        (Admit::Ancestors(_), true, false, None) => Unreached::NotAChat,
        (_, _, _, Some(why)) => Unreached::AppGone(why),
        (_, _, true, None) => Unreached::AppGone(format!(
            "the app at {} did not say which chat this process runs in",
            socket.map_or_else(String::new, |it| it.display().to_string())
        )),
        (_, true, false, None) => Unreached::AppGone(format!(
            "the app at {} names no chat this process runs in",
            socket.map_or_else(String::new, |it| it.display().to_string())
        )),
        (_, false, false, None) => {
            Unreached::AppGone("the app this process runs under holds no hook socket".to_owned())
        }
    }
}

/// What a listener must be to be believed.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Admit {
    /// One of these processes: the app above this one ([`Under::App`]).
    Ancestors(Vec<u32>),
    /// Any process of this user's, at a socket in a folder this process may not write
    /// ([`Under::Boxed`]).
    Unwritable,
}

/// What the app at one socket said to [`WhoAmI`].
enum Asked {
    Named(Reached),
    InNoChat,
    /// Older than the question: with the connection, where it answered something else and kept
    /// it open.
    Older(Option<UnixStream>),
}

/// Asks the app at `socket` which chat this process runs in. [`Unreached::AppGone`] where
/// nothing listens there; [`Unreached::NotTheApp`] where what listens is not the app.
fn asked_at(socket: &Path, admitted: &Admit, within: Duration) -> Result<Asked, Unreached> {
    let stream = match UnixStream::connect(socket) {
        Ok(stream) => stream,
        Err(why) => {
            return Err(Unreached::AppGone(format!(
                "nothing listens at {} ({why})",
                socket.display()
            )));
        }
    };
    admit(&stream, socket, admitted).map_err(|why| Unreached::NotTheApp(why.to_string()))?;
    match whoami(&stream, within) {
        Ok(Some(you)) => Ok(Asked::Named(Reached {
            chat: you.you_are,
            token: Some(ChatToken::from(you.token.as_str())),
            stream,
            socket: socket.to_path_buf(),
        })),
        Ok(None) => Ok(Asked::InNoChat),
        // The app turned the question away (it serves as many connections as it may): it said
        // nothing about this process, and is not asked again.
        Err(why) if why.kind() == io::ErrorKind::ConnectionRefused => {
            Err(Unreached::AppGone(format!(
                "the app at {} turned the question away ({why})",
                socket.display()
            )))
        }
        // An app older than the question ends the connection with the sentence it says to a
        // line it cannot read, or ends it unanswered; one that does not answer in time is
        // taken for one too, and a claim is put to it on a connection of its own. One that
        // answered it with something else is still listening on this one.
        Err(why) if why.kind() == io::ErrorKind::InvalidData => Ok(Asked::Older(Some(stream))),
        Err(_) => Ok(Asked::Older(None)),
    }
}

/// Writes [`WhoAmI`] on `stream` and reads the answer: `Some` where the app names the chat,
/// `None` where it says this process runs in none, an error otherwise.
fn whoami(stream: &UnixStream, within: Duration) -> io::Result<Option<YouAre>> {
    let mut line = serde_json::to_vec(&WhoAmI { whoami: true }).map_err(io::Error::other)?;
    line.push(b'\n');
    stream.set_write_timeout(Some(within))?;
    stream.set_read_timeout(Some(within))?;
    (&*stream).write_all(&line)?;
    // A byte at a time to the newline, so nothing past this answer is taken off the connection.
    let mut said = Vec::new();
    let mut one = [0u8; 1];
    loop {
        if (&*stream).read(&mut one)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the app closed the connection without answering",
            ));
        }
        if one[0] == b'\n' {
            break;
        }
        said.push(one[0]);
        if said.len() as u64 > super::A_REPORT_IS_AT_MOST {
            return Err(io::Error::other(
                "the answer was longer than any app writes",
            ));
        }
    }
    if let Ok(you) = serde_json::from_slice::<YouAre>(&said) {
        return Ok(Some(you));
    }
    match serde_json::from_slice::<Answer>(&said) {
        Ok(Answer::No { why }) if why == IN_NO_CHAT => Ok(None),
        // The sentence an older app says to a line it cannot read: it ends the connection.
        Ok(Answer::No { why }) if why == super::UNKNOWN_LINE => {
            Err(io::Error::other("the app is older than the question"))
        }
        // Any other refusal: the app read no question, and ends the connection.
        Ok(Answer::No { why }) => Err(io::Error::new(io::ErrorKind::ConnectionRefused, why)),
        // Anything else: an app that answered without reading the question, and listens on.
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the app answered something else",
        )),
    }
}

/// Whether `one` and `other` name the same socket: the same path once each is resolved.
fn same_socket(one: &Path, other: &Path) -> bool {
    let resolved = |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    resolved(one) == resolved(other)
}

/// A connection to the app at `socket`, authenticated as `admitted` says.
fn connect_app(socket: &Path, admitted: &Admit) -> io::Result<UnixStream> {
    let stream = UnixStream::connect(socket)?;
    admit(&stream, socket, admitted)
        .map_err(|why| io::Error::new(io::ErrorKind::PermissionDenied, why))?;
    Ok(stream)
}

/// Why what listens at a socket is not the app.
#[derive(Debug, thiserror::Error)]
pub enum NotTheApp {
    #[error("{0}")]
    NotThisUser(purlis_same_user::NotThisUser),
    #[error("the process listening at {path} (pid {pid}) is not the app this process runs under")]
    NotOurApp { pid: u32, path: String },
    #[error("the process listening at {path} is not the app this process runs under: {why}")]
    NotOurHost {
        why: purlis_same_user::NotOurHost,
        path: String,
    },
    #[error(
        "the socket at {path} is not in a folder a sandbox keeps this process from writing, so \
         it may be one this chat bound itself"
    )]
    Writable { path: String },
}

/// `Ok` where the process at the far end of `socket`, connected at `path`, is believed as
/// `admitted` says: this user's (the kernel's peer credentials: `SO_PEERCRED` on Linux,
/// `LOCAL_PEERCRED` and `LOCAL_PEERPID` on macOS), and then
///
/// - [`Admit::Ancestors`]: **one of the app's own processes above this one, holding the socket
///   itself** ([`purlis_same_user::admit_host`]). The peer pid alone is a number a listener
///   that has gone leaves behind, which a later process may be given; the process with it must
///   hold the listening socket now (or, inside Linux's sandbox, be the one the pidfd names).
/// - [`Admit::Unwritable`]: at a socket in a folder a sandbox keeps this process from writing
///   ([`may_not_write`]).
fn admit(socket: &UnixStream, path: &Path, admitted: &Admit) -> Result<(), NotTheApp> {
    let (uid, pid) = purlis_same_user::peer_process_of(socket)
        .map_err(|why| NotTheApp::NotThisUser(purlis_same_user::NotThisUser::Unidentified(why)))?;
    purlis_same_user::admit_peer(Ok(uid), purlis_same_user::Uid::effective())
        .map_err(NotTheApp::NotThisUser)?;
    match admitted {
        Admit::Ancestors(app) if app.contains(&pid) => {
            purlis_same_user::admit_host(socket, path, crate::forklock::output).map_err(|why| {
                NotTheApp::NotOurHost {
                    why,
                    path: path.display().to_string(),
                }
            })
        }
        Admit::Ancestors(_) => Err(NotTheApp::NotOurApp {
            pid,
            path: path.display().to_string(),
        }),
        Admit::Unwritable if may_not_write(path.parent().unwrap_or(Path::new("/"))) => Ok(()),
        Admit::Unwritable => Err(NotTheApp::Writable {
            path: path.display().to_string(),
        }),
    }
}

/// **Whether a sandbox's rule keeps this process from writing in `folder`**, which nothing the
/// sandbox runs can lift, so nothing it runs bound a socket there.
///
/// A folder this process merely cannot write now proves nothing: a chat can `chmod 555` a folder
/// of its own, set a flag on it (`chflags uchg`), or put an access list on it, after binding a
/// socket there. So `folder` must be this user's, its mode must let its owner make a file in it,
/// no flag (macOS) or access list (Linux) may close it, and making a file there, tried with a
/// name no one else uses and removed at once where it was made, must still be refused by the
/// error a sandbox gives: Seatbelt's `EPERM` on macOS, Landlock's `EACCES` on Linux. A
/// read-only mount is not one: a chat can make one of its own folders in a namespace of its own.
pub fn may_not_write(folder: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(meta) = std::fs::metadata(folder) else {
        return false;
    };
    let ours = meta.is_dir() && meta.uid() == purlis_same_user::Uid::effective().as_raw();
    if !ours || !its_mode_lets_its_owner_write(meta.mode()) || closed_by_more_than_its_mode(folder)
    {
        return false;
    }
    let mut bytes = [0u8; 8];
    if getrandom::fill(&mut bytes).is_err() {
        return false;
    }
    let name: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    let probe = folder.join(format!(".purlis-may-write-{}-{name}", std::process::id()));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            false
        }
        Err(why) => why.raw_os_error().is_some_and(a_sandbox_s_refusal),
    }
}

/// Whether a folder's `mode` lets its owner make a file in it: write and search.
pub fn its_mode_lets_its_owner_write(mode: u32) -> bool {
    mode & 0o300 == 0o300
}

/// Whether the error number `errno` is the refusal a sandbox gives for making a file: Seatbelt's
/// `EPERM` on macOS (an access list's is `EACCES`), Landlock's `EACCES` or a filter's `EPERM` on
/// Linux.
pub fn a_sandbox_s_refusal(errno: i32) -> bool {
    if cfg!(target_os = "macos") {
        errno == libc::EPERM
    } else {
        errno == libc::EACCES || errno == libc::EPERM
    }
}

/// Whether something besides its mode closes `folder` to writing that its owner may set: a
/// flag that makes it unchangeable or append-only (macOS), or an access list (Linux).
fn closed_by_more_than_its_mode(folder: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        // `UF_IMMUTABLE`, `UF_APPEND`, `SF_IMMUTABLE`, `SF_APPEND`.
        const CLOSING: u32 = 0x0000_0002 | 0x0000_0004 | 0x0002_0000 | 0x0004_0000;
        std::fs::metadata(folder).map_or(true, |meta| meta.st_flags() & CLOSING != 0)
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let mut names = vec![0u8; 4096];
        match rustix::fs::listxattr(folder, &mut names) {
            Ok(len) => names[..len]
                .split(|byte| *byte == 0)
                .any(|name| name.starts_with(b"system.posix_acl")),
            Err(_) => true,
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "android")))]
    {
        let _ = folder;
        true
    }
}

/// What a program named `name` is to an ask, in a shipped build ([`program_of`]): nothing but its
/// name decides.
pub fn program_named(name: &str) -> Ran {
    let named = |of: crate::names::Name| {
        std::iter::once(of.write)
            .chain(of.reads.iter().copied())
            .any(|it| it == name)
    };
    if named(crate::names::APP_BINARY) {
        Ran::App
    } else if named(crate::names::BINARY) {
        Ran::Purlis
    } else {
        Ran::Other
    }
}

/// **What `exe`, an ancestor's program, is to an ask** (#6995), read from that program alone:
/// the app's program by its name (`purlis-app`, or the name it had), `purlis` by its, or neither.
/// Never relative to where this process's own program is, so a copy of `purlis` in a folder a
/// chat writes neither vouches for a program beside it nor makes the real app a stranger. A
/// chat's own copy of the app's program runs below the chat's harness, never above the app, and
/// the app is the topmost ([`under_of`]). The code signature is not compared: a development
/// build is signed ad hoc, with no team to compare.
///
/// **A test build** (`test-support`, which only `cargo test` turns on) believes only programs
/// of its own build folder, so an app above the test run never stands in for the one a test
/// plays, and takes any program there for the app's, where a test binary plays it. A test of
/// this very trust sets `REACH_TRUSTS_ONLY_THE_APP` (read only in a test build), and plays the
/// app from a copy of itself named as the app's program is.
pub fn program_of(exe: &Path) -> Ran {
    let ran = program_named(exe.file_name().and_then(|name| name.to_str()).unwrap_or(""));
    if !cfg!(feature = "test-support") {
        return ran;
    }
    let build = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .ok()
        .and_then(|me| me.parent().map(Path::to_path_buf));
    let exe = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_owned());
    match build {
        Some(build) if exe.starts_with(&build) => {
            if ran == Ran::Other && std::env::var_os("REACH_TRUSTS_ONLY_THE_APP").is_none() {
                Ran::App
            } else {
                ran
            }
        }
        _ => Ran::Other,
    }
}

#[cfg(test)]
mod tests;
