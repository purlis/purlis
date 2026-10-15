// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What restarting a chat ends** (#1986, ruling Q63): the programs its commands started that
//! are running now, and of them, each one listening on a TCP port, so an Ask that restarts the
//! chat says beforehand "its node on port 5173 will stop".
//!
//! Read from the operating system's own process table (`ps`) and its list of listening sockets
//! (`lsof`), from the chat's program down, and nothing else: never from what the chat printed.
//! Where either cannot be read, purlis cannot tell, and says so rather than guess.
//!
//! **What its commands started** is what runs below a shell the chat's program started, the
//! shell itself not counted: a harness runs a command through a shell, and starts its own
//! servers (an MCP server) directly, which start again with it. Every program below the chat
//! that listens on a port is named, whoever started it.

use std::collections::BTreeSet;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

/// What a restart of a chat would end, as purlis can see it now.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ends {
    /// Each program below the chat listening on a TCP port, by port.
    pub listening: Vec<Listening>,
    /// How many other programs its commands started are running.
    pub others: u32,
}

/// One program listening on one port.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Listening {
    /// Its program's name, without its folder.
    pub program: String,
    pub port: u16,
}

/// One process below a chat's program, as `ps` listed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Below {
    pub pid: u32,
    pub parent: u32,
    /// Its program's name, without its folder.
    pub program: String,
}

/// The shells a harness runs a command in.
const SHELLS: &[&str] = &["sh", "bash", "zsh", "dash", "fish", "ksh"];

/// Every process below `root` in `listing`, the output of `ps -A -o pid=,ppid=,comm=`, nearest
/// first. A line that is not a process is passed over.
pub fn below(listing: &str, root: u32) -> Vec<Below> {
    let all: Vec<Below> = listing.lines().filter_map(one_process).collect();
    let mut found: Vec<Below> = Vec::new();
    let mut parents = vec![root];
    while let Some(parent) = parents.pop() {
        for one in all
            .iter()
            .filter(|one| one.parent == parent && one.pid != root)
        {
            if !found.iter().any(|seen| seen.pid == one.pid) {
                parents.push(one.pid);
                found.push(one.clone());
            }
        }
    }
    found
}

/// One line of `ps -A -o pid=,ppid=,comm=`: two numbers, then the program, which may hold
/// spaces.
fn one_process(line: &str) -> Option<Below> {
    let line = line.trim_start();
    let (pid, rest) = line.split_once(char::is_whitespace)?;
    let rest = rest.trim_start();
    let (parent, program) = rest.split_once(char::is_whitespace)?;
    let program = program.trim();
    let name = program.rsplit('/').next().unwrap_or(program);
    if name.is_empty() {
        return None;
    }
    Some(Below {
        pid: pid.parse().ok()?,
        parent: parent.parse().ok()?,
        program: name.to_owned(),
    })
}

/// The process and port of each listening socket in `text`, the output of
/// `lsof -nP -a -iTCP -sTCP:LISTEN -p <pids> -F pn`: a `p<pid>` line, then its `n<address>:<port>`
/// lines.
pub fn listening_in(text: &str) -> Vec<(u32, u16)> {
    let mut found = Vec::new();
    let mut pid = None;
    for line in text.lines() {
        if let Some(number) = line.strip_prefix('p') {
            pid = number.parse().ok();
        } else if let (Some(name), Some(pid)) = (line.strip_prefix('n'), pid)
            && let Some(port) = name.rsplit(':').next().and_then(|port| port.parse().ok())
        {
            found.push((pid, port));
        }
    }
    found
}

/// What a restart ends, of the processes `below` a chat with `listening` sockets among them.
pub fn ends_of(below: &[Below], listening: &[(u32, u16)]) -> Ends {
    let mut named: BTreeSet<(u16, String)> = BTreeSet::new();
    for (pid, port) in listening {
        if let Some(one) = below.iter().find(|one| one.pid == *pid) {
            named.insert((*port, one.program.clone()));
        }
    }
    let listens = |pid: u32| listening.iter().any(|(one, _)| *one == pid);
    let is_shell = |one: &Below| SHELLS.contains(&one.program.as_str());
    let under_a_shell = |one: &Below| {
        let mut parent = one.parent;
        while let Some(up) = below.iter().find(|up| up.pid == parent) {
            if is_shell(up) {
                return true;
            }
            parent = up.parent;
        }
        false
    };
    let others = below
        .iter()
        .filter(|one| !is_shell(one) && !listens(one.pid) && under_a_shell(one))
        .count();
    Ends {
        listening: named
            .into_iter()
            .map(|(port, program)| Listening { program, port })
            .collect(),
        others: u32::try_from(others).unwrap_or(u32::MAX),
    }
}

/// **What restarting the chat whose program is `pid` ends now**, or `None` where purlis cannot
/// tell: the process table or the listening sockets could not be read within [`DEADLINE`].
pub fn of(pid: u32) -> Option<Ends> {
    let (status, listing) = within(
        Command::new("/bin/ps").args(["-A", "-o", "pid=,ppid=,comm="]),
        DEADLINE,
    )?;
    if status != Some(0) {
        return None;
    }
    let below = below(&listing, pid);
    if below.is_empty() {
        return Some(Ends::default());
    }
    let pids: Vec<String> = below.iter().map(|one| one.pid.to_string()).collect();
    let lsof = ["/usr/sbin/lsof", "/usr/bin/lsof"]
        .into_iter()
        .find(|path| std::path::Path::new(path).exists())?;
    // `-b` and `-w`: no kernel call that can block, and no warnings on its output.
    let (status, listed) = within(
        Command::new(lsof).args([
            "-b",
            "-w",
            "-nP",
            "-a",
            "-iTCP",
            "-sTCP:LISTEN",
            "-p",
            &pids.join(","),
            "-F",
            "pn",
        ]),
        DEADLINE,
    )?;
    Some(ends_of(&below, &lsof_answer(status, &listed)?))
}

/// How long `ps` and `lsof` may each take: the Ask is drawn while they run, and says purlis
/// cannot tell when either takes longer.
pub const DEADLINE: Duration = Duration::from_secs(2);

/// The listening sockets `lsof` answered with exit status `status` and `listed` on its standard
/// output, or `None` where it failed. What it printed is read whatever its status: it answers 1
/// when it found nothing to list, and also when one of the processes it was asked about ended
/// meanwhile, after listing the others. Nothing printed and a status past 1 is a failure.
pub fn lsof_answer(status: Option<i32>, listed: &str) -> Option<Vec<(u32, u16)>> {
    if !listed.is_empty() || matches!(status, Some(0 | 1)) {
        Some(listening_in(listed))
    } else {
        None
    }
}

/// `command`'s exit status and standard output, where it ends within `deadline`; `None`
/// otherwise, and a command still running at the deadline is killed. Its output is read while
/// it runs, so a long one cannot fill the pipe and stop it.
pub fn within(command: &mut Command, deadline: Duration) -> Option<(Option<i32>, String)> {
    let mut child = crate::forklock::spawn(
        no_env(command)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null()),
    )
    .ok()?;
    let mut out = child.stdout.take()?;
    let reading = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        out.read_to_end(&mut bytes).map(|_| bytes)
    });
    let until = Instant::now() + deadline;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let bytes = reading.join().ok()?.ok()?;
    Some((status.code(), String::from_utf8_lossy(&bytes).into_owned()))
}

/// `command` with no environment: what it reads is the operating system's, never purlis's own.
fn no_env(command: &mut Command) -> &mut Command {
    command.env_clear()
}
