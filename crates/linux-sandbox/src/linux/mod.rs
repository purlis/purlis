// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The wrap itself: the supervisor's start, and the chat's.
//!
//! `purlis _sandbox --profile <piece>… -- <program> <args>…` is the supervisor. It reads the
//! profile, holds itself to it with Landlock, and starts `purlis _sandbox --chat <socket> --
//! <program> <args>…`: the chat's side, which applies its own Landlock layer and the seccomp
//! filters to itself, hands the supervisor the filter's listener over `<socket>`, and becomes
//! the program. The supervisor then serves the chat's calls until the chat ends, and ends with
//! its status.
//!
//! Neither side starts with the chat's environment (#2460): each reads the one it carries for
//! the program ([`crate::carried`]) and refuses to start anything where its own is not what
//! purlis built; the chat's side is started with it carried again, and the program alone is
//! given it.

mod filter;
mod layers;
mod notify;
mod supervisor;
mod walk;

use std::ffi::OsString;
use std::io::{IoSlice, IoSliceMut};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::Arc;

use nix::sys::signal::{SigSet, SigmaskHow, Signal, kill, pthread_sigmask};
use nix::sys::socket::{
    AddressFamily, ControlMessage, ControlMessageOwned, MsgFlags, SockFlag, SockType, SockaddrIn,
    getsockname, recvmsg, sendmsg, socketpair,
};
use nix::sys::wait::WaitStatus;
use nix::unistd::Pid;

pub use layers::abi;

use crate::profile::Profile;

/// The word that starts the chat's side, after [`crate::COMMAND`].
const CHAT: &str = "--chat";

/// How many calls the supervisor serves at once.
const SERVERS: usize = 16;

/// The longest a wrap whose chat has ended waits for its refusals to be told.
const TOLD_WITHIN: std::time::Duration = std::time::Duration::from_secs(1);

/// How many ports the system picks for a wrap's chat on 127.0.0.1 (#3396): as many test servers
/// as it can have listening at once.
const POOL: usize = 256;

/// The status a wrap ends with when it could not start the chat.
const NOT_STARTED: i32 = 126;

/// The environment the program is given, as purlis carried it.
type Env = Vec<(OsString, OsString)>;

/// `purlis _sandbox`, given the words after it: the status to exit with.
/// Each call the supervisor refuses is told to what `tell` makes of the program's environment,
/// where it makes anything.
pub fn entry(
    words: &[String],
    tell: impl FnOnce(&[(OsString, OsString)]) -> Option<crate::refusal::Tell>,
) -> i32 {
    let env = match crate::carried::read(std::env::vars_os()) {
        Ok(env) => env,
        Err(why) => return refuse(&why.to_string()),
    };
    if words.first().map(String::as_str) == Some(CHAT) {
        return chat_side(&words[1..], env);
    }
    let Some((profile, program)) = crate::read_line(words) else {
        return refuse("the sandbox was started without a profile and a program");
    };
    // Read now, while nothing holds the wrap; its teller is started once the wrap holds itself.
    let tell = tell(&env);
    match supervise(&profile, &program, env, tell) {
        Ok(code) => code,
        Err(why) => refuse(&why),
    }
}

/// **`purlis _no-namespaces -- <program> <args>…`** (#3298, D-2050-25): starts `program` with
/// `args`, in this process, unable to make a namespace, and every program it starts with it
/// ([`filter::namespaces_refused`]). Its environment is its own, as it was started. The status to
/// exit with where it could not be started.
pub(crate) fn without_namespaces(program: &std::ffi::OsStr, args: &[OsString]) -> i32 {
    // A filter a process applies to itself needs no new privileges: a runner's container sets
    // it, and it is set here too, so the filter never depends on where it runs.
    let held = nix::sys::prctl::set_no_new_privs()
        .map_err(|why| format!("no new privileges could not be set: {why}"))
        .and_then(|()| filter::namespaces_refused())
        .and_then(|programs| {
            programs.iter().try_for_each(|program| {
                seccompiler::apply_filter(program).map_err(|why| {
                    format!("the filter that leaves it no namespace could not be applied: {why}")
                })
            })
        });
    match held {
        Ok(()) => {
            let why = Command::new(program).args(args).exec();
            eprintln!(
                "purlis: {} could not be started: {why}",
                std::path::Path::new(program).display()
            );
            127
        }
        Err(why) => refuse(&why),
    }
}

fn refuse(why: &str) -> i32 {
    eprintln!("purlis: {why}, so nothing was started.");
    NOT_STARTED
}

fn supervise(
    profile: &str,
    program: &[String],
    env: Env,
    tell: Option<crate::refusal::Tell>,
) -> Result<i32, String> {
    let profile = Profile::parse(profile).map_err(|why| why.to_string())?;
    let abi = abi();
    if abi < crate::MIN_ABI {
        eprintln!(
            "purlis: {}, and this kernel's is version {abi}, so nothing was started. {}",
            crate::NEEDS_LANDLOCK,
            crate::landlock_way_on(true)
        );
        return Ok(NOT_STARTED);
    }
    notify::check_sizes().map_err(|why| why.to_string())?;
    // A process of the chat whose parent ends is the supervisor's, so it stays below it.
    nix::sys::prctl::set_child_subreaper(true)
        .map_err(|why| format!("the wrap could not keep the chat's processes: {why}"))?;
    // Picked before the wrap holds itself, which names each port it may bind and reach.
    let picked = crate::loopback::Picked::new(picked_ports(POOL));
    layers::supervisor(&profile.reach(), picked.ports())
        .map_err(|why| format!("Landlock could not be applied to the wrap: {why}"))?;

    let (ours, theirs) = socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )
    .map_err(|why| why.to_string())?;
    nix::fcntl::fcntl(
        &ours,
        nix::fcntl::FcntlArg::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
    )
    .map_err(|why| why.to_string())?;
    let me = std::env::current_exe().map_err(|why| why.to_string())?;
    let child = Command::new(me)
        .arg(crate::COMMAND)
        .arg(CHAT)
        .arg(theirs.as_raw_fd().to_string())
        .arg(crate::END)
        .args(program)
        .env_clear()
        .envs(crate::carried::carried(env))
        .spawn()
        .map_err(|why| why.to_string())?;
    drop(theirs);
    // The supervisor makes files with the mode the chat asked for, less the chat's own mask,
    // so its own is none; set after the chat started, which keeps the mask it was given.
    nix::sys::stat::umask(nix::sys::stat::Mode::empty());

    // Signals the terminal sends reach the chat's group itself; an ending asked of the wrap is
    // passed on to the chat.
    let mut held = SigSet::empty();
    for signal in [
        Signal::SIGINT,
        Signal::SIGQUIT,
        Signal::SIGTERM,
        Signal::SIGHUP,
    ] {
        held.add(signal);
    }
    pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&held), None).map_err(|why| why.to_string())?;
    let pid = Pid::from_raw(i32::try_from(child.id()).unwrap_or(0));
    std::thread::spawn(move || {
        while let Ok(signal) = held.wait() {
            if matches!(signal, Signal::SIGTERM | Signal::SIGHUP) {
                let _ = kill(pid, signal);
            }
        }
    });

    // Started only now, so its thread, and every thread it starts, is held as this one is: by
    // the wrap's own Landlock layer, and with the signals above left to the waiter.
    let told = tell.map(|tell| Arc::new(crate::refusal::Teller::new(tell)));
    if let Some(listener) = received(&ours) {
        match supervisor::Supervisor::new(listener, profile, told.clone(), picked) {
            Ok(supervisor) => {
                let supervisor = Arc::new(supervisor);
                for _ in 0..SERVERS {
                    let supervisor = Arc::clone(&supervisor);
                    std::thread::spawn(move || supervisor.serve());
                }
            }
            Err(why) => {
                let _ = kill(pid, Signal::SIGKILL);
                return Err(why.to_string());
            }
        }
    } else {
        // No listener: nothing would serve the chat's calls, so it does not run.
        let _ = kill(pid, Signal::SIGKILL);
    }
    // Every process of the chat whose parent ended is the supervisor's to reap; the chat's
    // own ending is its status.
    drop(child);
    let ended = loop {
        match nix::sys::wait::waitpid(None, None) {
            Ok(WaitStatus::Exited(at, code)) if at == pid => break Ok(code),
            Ok(WaitStatus::Signaled(at, signal, _)) if at == pid => {
                break Ok(128 + signal as i32);
            }
            Ok(_) | Err(nix::errno::Errno::EINTR) => {}
            Err(why) => break Err(why.to_string()),
        }
    };
    // The refusals that ended the chat are told before the wrap goes, for a moment at most.
    if let Some(told) = &told {
        told.flush(TOLD_WITHIN);
    }
    ended
}

/// **Up to `count` ports the system picks on 127.0.0.1 now** (#3396), each told apart by holding
/// every socket bound until the last is; let go again once read. Fewer where the system runs out
/// of descriptors or ports.
fn picked_ports(count: usize) -> Vec<u16> {
    let mut held = Vec::with_capacity(count);
    let mut ports = Vec::with_capacity(count);
    for _ in 0..count {
        let Ok(socket) = nix::sys::socket::socket(
            AddressFamily::Inet,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            None,
        ) else {
            break;
        };
        let loopback = SockaddrIn::new(127, 0, 0, 1, 0);
        if nix::sys::socket::bind(socket.as_raw_fd(), &loopback).is_err() {
            break;
        }
        let Ok(at) = getsockname::<SockaddrIn>(socket.as_raw_fd()) else {
            break;
        };
        ports.push(at.port());
        held.push(socket);
    }
    ports
}

/// The listener the chat's side sends, or `None` where it ended first.
fn received(socket: &OwnedFd) -> Option<OwnedFd> {
    let mut byte = [0u8; 1];
    let mut space = nix::cmsg_space!([std::os::fd::RawFd; 1]);
    let mut iov = [IoSliceMut::new(&mut byte)];
    let message = recvmsg::<()>(
        socket.as_raw_fd(),
        &mut iov,
        Some(&mut space),
        MsgFlags::MSG_CMSG_CLOEXEC,
    )
    .ok()?;
    for control in message.cmsgs().ok()? {
        if let ControlMessageOwned::ScmRights(fds) = control
            && let Some(fd) = fds.first()
        {
            return adopt(*fd);
        }
    }
    None
}

/// The descriptor `fd`, just received with `SCM_RIGHTS`, as one this process owns.
fn adopt(fd: std::os::fd::RawFd) -> Option<OwnedFd> {
    notify::adopt(fd).ok()
}

/// The chat's side: its own layer and filters, the listener to the supervisor, then the
/// program.
fn chat_side(words: &[String], env: Env) -> i32 {
    let (Some(socket), Some(end)) = (words.first(), words.get(1)) else {
        return refuse("the sandbox's chat side was started without its socket");
    };
    if end != crate::END || words.len() < 3 {
        return refuse("the sandbox's chat side was started without a program");
    }
    let program = &words[2..];
    match restrict_and_hand_over(socket) {
        Ok(()) => {
            let why = Command::new(&program[0])
                .args(&program[1..])
                .env_clear()
                .envs(env)
                .exec();
            eprintln!("purlis: {} could not be started: {why}", program[0]);
            127
        }
        Err(why) => refuse(&why),
    }
}

fn restrict_and_hand_over(socket: &str) -> Result<(), String> {
    let socket: std::os::fd::RawFd = socket.parse().map_err(|_| "a socket number".to_owned())?;
    let socket = notify::adopt(socket).map_err(|why| why.to_string())?;
    // Ended with the supervisor, so the chat never outlives what serves its calls.
    nix::sys::prctl::set_pdeathsig(Signal::SIGKILL)
        .map_err(|why| format!("the chat could not be tied to the wrap: {why}"))?;
    layers::chat().map_err(|why| format!("Landlock could not be applied to the chat: {why}"))?;
    // The supervised filter first: the refusing ones refuse a listener, the chat's own among
    // them. A refusal outranks a notification whichever filter says it.
    let listener = notify::install(&filter::supervised()?)
        .map_err(|why| format!("the chat's supervised filter could not be applied: {why}"))?;
    for program in filter::refused()? {
        seccompiler::apply_filter(&program)
            .map_err(|why| format!("the chat's seccomp filter could not be applied: {why}"))?;
    }
    let fds = [listener.as_raw_fd()];
    sendmsg::<()>(
        socket.as_raw_fd(),
        &[IoSlice::new(&[0])],
        &[ControlMessage::ScmRights(&fds)],
        MsgFlags::empty(),
        None,
    )
    .map_err(|why| why.to_string())?;
    drop(listener);
    drop(socket);
    Ok(())
}
