// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's server dials in** (#3316): the hook socket's `serve` line, and what becomes of
//! the connection it came on.
//!
//! A Claude Code chat can make no socket of its own and bind no port, so a server it runs
//! through `purlis listen` (its Node shim, in dial mode) connects to the chat's hook socket,
//! which its sandbox lets it reach, and writes one line: `{"chat": N, "serve": PORT}` with the
//! chat's token. The line is admitted as every line is (the token, and the process that sent
//! it running inside the chat). Then whoever hears it ([`super::Hearing::serve`]) checks that the
//! chat's door on that port is open, and either parks the connection as an idle slot of that
//! door and answers [`parked`], or answers [`refused`] and closes it. The door later writes its
//! go line on a parked slot (`crate::sandbox::door::GO`) and carries one browser request over it.
//!
//! **Where the hook socket is the session host's** (#2000), the host admits the line and hands
//! the connection on to the window, which holds the doors: over a socket of the window's own
//! beside the hook socket ([`beside`]), one connection a slot, its descriptor passed with the
//! ask ([`hand_on`], [`Receiving`]). Only the process that holds the hook socket's listener, the
//! session host itself, is read there: the line it hands on was admitted by its token already,
//! and no other process, of this user or not, can hand the window a slot. No chat's sandbox
//! reaches that socket either. A slot the window is not there to take is refused.
//!
//! Nothing here hands a chat a descriptor or binds anything for it: the chat only ever holds the
//! connection it opened itself.

use std::io;

/// A `serve` line: chat `chat`'s server asks to be reached on port `serve` through its door.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ServeAsk {
    pub chat: u32,
    pub serve: u16,
}

/// The connection a `serve` line came on.
#[cfg(unix)]
pub type Slot = std::os::unix::net::UnixStream;
#[cfg(not(unix))]
pub type Slot = std::convert::Infallible;

/// What a slot is told where nothing here parks slots.
pub const NOTHING_SERVES: &str = "this app parks no server's connection";

/// What a slot is told where its sender wrote more than its one line before it was answered.
pub const NOT_ONE_LINE: &str = "a serve line is sent alone, and its answer waited for";

/// What a slot is told where the window that holds the chat's door could not be reached.
pub const NO_WINDOW: &str = "the window that holds this chat's door could not take it";

/// The name of the window's own socket a session host hands slots on over, beside the hook
/// socket.
pub const SLOTS_SOCKET: &str = "slots.sock";

/// **Where the window takes slots a session host hands on** (#3316): [`SLOTS_SOCKET`] in the
/// folder of the hook socket at `hook_socket`.
pub fn beside(hook_socket: &std::path::Path) -> std::path::PathBuf {
    hook_socket.with_file_name(SLOTS_SOCKET)
}

/// What hears a `serve` line where nothing parks slots: every one refused.
pub fn refusing() -> super::Serves {
    Box::new(refuse_unparked)
}

#[cfg(unix)]
fn refuse_unparked(_: ServeAsk, slot: Slot) {
    refused(slot, NOTHING_SERVES);
}

#[cfg(not(unix))]
fn refuse_unparked(_: ServeAsk, slot: Slot) {
    match slot {}
}

/// **Tells a slot it is parked**: the receipt every hook line is answered with,
/// `{"taken": true}`. The door's go line comes on it later. A door's own slots are told so as
/// they are parked (`crate::sandbox::door::Slots::park`); this is for whatever else parks one.
#[cfg(unix)]
pub fn parked(slot: &mut Slot) -> io::Result<()> {
    use std::io::Write as _;
    slot.write_all(crate::sandbox::door::PARKED.as_bytes())
}

/// **Tells a slot why it is not parked, and closes it**: `{"taken": false, "refused": why}`.
/// Its server then listens as it would without purlis, and is refused that.
#[cfg(unix)]
pub fn refused(mut slot: Slot, why: &str) {
    use std::io::Write as _;
    if let Ok(mut said) = serde_json::to_vec(&serde_json::json!({
        "taken": false,
        "refused": why,
    })) {
        said.push(b'\n');
        let _ = slot.write_all(&said);
    }
    let _ = slot.shutdown(std::net::Shutdown::Both);
}

/// **Hands `slot` on to the window at `path`** (a session host's half): the ask as one line,
/// with the slot's descriptor beside it. The window answers the slot itself. Err where the
/// window could not be reached, and the slot is then the caller's to refuse.
#[cfg(unix)]
pub fn hand_on(path: &std::path::Path, ask: &ServeAsk, slot: &Slot) -> io::Result<()> {
    use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};
    use std::os::fd::AsFd as _;
    let window = std::os::unix::net::UnixStream::connect(path)?;
    window.set_write_timeout(Some(std::time::Duration::from_secs(2)))?;
    window.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
    let mut line = serde_json::to_vec(ask).map_err(io::Error::other)?;
    line.push(b'\n');
    let fds = [slot.as_fd()];
    let mut space = [std::mem::MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    if !control.push(SendAncillaryMessage::ScmRights(&fds)) {
        return Err(io::Error::other("the slot's descriptor did not fit"));
    }
    let sent = sendmsg(
        &window,
        &[io::IoSlice::new(&line)],
        &mut control,
        SendFlags::empty(),
    )?;
    if sent != line.len() {
        return Err(io::Error::other("the ask was not sent whole"));
    }
    // Held until the window has taken it: it closes its end once it has.
    let mut end = [0u8; 1];
    let _ = std::io::Read::read(&mut &window, &mut end);
    Ok(())
}

/// **The window's socket a session host hands slots on over** (#3316), read while this is held.
#[cfg(unix)]
pub struct Receiving {
    path: std::path::PathBuf,
    /// Which file at `path` is this one's: the one `bind` made, so a later one is never removed.
    file: (u64, u64),
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    waking: Option<std::os::unix::net::UnixStream>,
    reading: Option<std::thread::JoinHandle<()>>,
}

#[cfg(unix)]
impl Receiving {
    /// **Binds [`beside`] the hook socket at `hook_socket`**, in its folder, which is made this
    /// user's alone where it is not; every slot handed on is handed to `each`. Only the process
    /// that holds the hook socket's listener is read ([`HookHolder`]).
    pub fn bind(hook_socket: &std::path::Path, each: super::Serves) -> io::Result<Receiving> {
        Self::bind_admitting(hook_socket, each, HookHolder::admitting(hook_socket))
    }

    /// [`Self::bind`], admitting whom `admits` admits: for the tests, which hand slots on from
    /// their own process.
    fn bind_admitting(
        hook_socket: &std::path::Path,
        each: super::Serves,
        mut admits: impl FnMut(&std::os::unix::net::UnixStream) -> bool + Send + 'static,
    ) -> io::Result<Receiving> {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
        let path = beside(hook_socket);
        if let Some(folder) = path.parent() {
            purlis_same_user::private_directory(folder)?;
        }
        match std::fs::remove_file(&path) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
            _ => {}
        }
        let listener = std::os::unix::net::UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        let made = std::fs::symlink_metadata(&path)?;
        let file = (made.dev(), made.ino());
        listener.set_nonblocking(true)?;
        let (woken, waking) = std::os::unix::net::UnixStream::pair()?;
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopped = std::sync::Arc::clone(&stop);
        let reading = std::thread::Builder::new()
            .name("purlis-slots".into())
            .spawn(move || {
                use rustix::event::{PollFd, PollFlags, poll};
                loop {
                    let mut waits = [
                        PollFd::new(&listener, PollFlags::IN),
                        PollFd::new(&woken, PollFlags::IN),
                    ];
                    if poll(&mut waits, None).is_err() {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        continue;
                    }
                    if stopped.load(std::sync::atomic::Ordering::SeqCst)
                        || !waits[1].revents().is_empty()
                    {
                        return;
                    }
                    let Ok((handing, _)) = listener.accept() else {
                        // Out of descriptors, most likely: the socket stays readable, so wait a
                        // little rather than spin.
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        continue;
                    };
                    if !admits(&handing) {
                        continue;
                    }
                    if let Some((ask, slot)) = taken_from(&handing) {
                        each(ask, slot);
                    }
                }
            })?;
        Ok(Receiving {
            path,
            file,
            stop,
            waking: Some(waking),
            reading: Some(reading),
        })
    }
}

#[cfg(unix)]
impl Drop for Receiving {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        drop(self.waking.take());
        if let Some(reading) = self.reading.take() {
            let _ = reading.join();
        }
        use std::os::unix::fs::MetadataExt as _;
        let its = std::fs::symlink_metadata(&self.path)
            .is_ok_and(|now| (now.dev(), now.ino()) == self.file);
        if its {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// **Whether a connection to the window's slots socket comes from the session host** (#3316):
/// this user's, and the process that holds the hook socket's listener now, read from the
/// kernel's own tables as `purlis_same_user::admit_host` reads a host. The peer's pid is the
/// process that connected, which is alive while the connection is open. One found to hold it is
/// taken at its word for [`A_HOST_IS_KNOWN_FOR`], so not every request pays for the read.
#[cfg(unix)]
struct HookHolder {
    hook: std::path::PathBuf,
    owner: purlis_same_user::Uid,
    known: Option<(u32, std::time::Instant)>,
}

/// How long a process found holding the hook socket is taken for the session host.
#[cfg(unix)]
const A_HOST_IS_KNOWN_FOR: std::time::Duration = std::time::Duration::from_secs(60);

#[cfg(unix)]
impl HookHolder {
    fn admitting(
        hook: &std::path::Path,
    ) -> impl FnMut(&std::os::unix::net::UnixStream) -> bool + Send + use<> {
        let mut holder = HookHolder {
            hook: hook.to_path_buf(),
            owner: purlis_same_user::Uid::effective(),
            known: None,
        };
        move |handing| holder.admits(handing)
    }

    fn admits(&mut self, handing: &std::os::unix::net::UnixStream) -> bool {
        let Ok((uid, pid)) = purlis_same_user::peer_process_of(handing) else {
            return false;
        };
        if purlis_same_user::admit_peer(Ok(uid), self.owner).is_err() {
            return false;
        }
        if self
            .known
            .is_some_and(|(known, at)| known == pid && at.elapsed() < A_HOST_IS_KNOWN_FOR)
        {
            return true;
        }
        let holds = purlis_same_user::holds_a_socket_at(pid, &self.hook, crate::forklock::output)
            .unwrap_or(false);
        self.known = holds.then(|| (pid, std::time::Instant::now()));
        holds
    }
}

/// **The ask and the slot a host handed on over `handing`**: one line, with one descriptor
/// beside it, which must be a connected unix socket. None for anything else; whatever
/// descriptors came are closed.
#[cfg(unix)]
fn taken_from(handing: &std::os::unix::net::UnixStream) -> Option<(ServeAsk, Slot)> {
    use rustix::net::{RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, recvmsg};
    let _ = handing.set_nonblocking(false);
    let _ = handing.set_read_timeout(Some(std::time::Duration::from_secs(2)));
    let mut line = [0u8; 256];
    let mut space = [std::mem::MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
    let mut control = RecvAncillaryBuffer::new(&mut space);
    let got = recvmsg(
        handing,
        &mut [io::IoSliceMut::new(&mut line)],
        &mut control,
        RecvFlags::empty(),
    )
    .ok()?;
    // Every descriptor that came is owned here at once, so each is closed whatever is made of it.
    let mut fds = Vec::new();
    for message in control.drain() {
        if let RecvAncillaryMessage::ScmRights(came) = message {
            fds.extend(came);
        }
    }
    let [fd] = <[std::os::fd::OwnedFd; 1]>::try_from(fds).ok()?;
    // Never handed to a program this process starts (macOS takes no flag for it on the receive).
    rustix::io::fcntl_setfd(&fd, rustix::io::FdFlags::CLOEXEC).ok()?;
    let said = std::str::from_utf8(line.get(..got.bytes)?)
        .ok()?
        .strip_suffix('\n')?;
    let ask: ServeAsk = serde_json::from_str(said).ok()?;
    let slot = std::os::unix::net::UnixStream::from(fd);
    // A connected unix socket, and nothing else.
    slot.peer_addr().ok()?;
    Some((ask, slot))
}

#[cfg(all(test, unix))]
mod tests;
