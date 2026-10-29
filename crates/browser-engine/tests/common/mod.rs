// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A host in a process of its own, for tests of what happens when it ends.
//!
//! The host is this same test binary, run again with only one test selected and
//! `PURLIS_ENGINE_TEST_HOST_PART` saying which part it plays. It starts what it was asked to,
//! prints one line naming the process group and its members, and waits to be ended.

#![allow(dead_code)]

pub mod drive;
pub mod fixture;
pub mod site;

use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::sys::signal::{Signal, kill, killpg};
use nix::unistd::Pid;

const PART: &str = "PURLIS_ENGINE_TEST_HOST_PART";

/// A host started in a process of its own, and what it said it started.
pub struct HostPart {
    child: Child,
    pub group: i32,
    pub member: u32,
    pub others: Vec<u32>,
    /// Whether dropping this still has to end the group: not once the test has ended the host
    /// on purpose, because then what ends the group is what the test is watching.
    armed: bool,
}

impl HostPart {
    /// Whether this process is the host's part named `part`.
    pub fn is_me(part: &str) -> bool {
        std::env::var(PART).as_deref() == Ok(part)
    }

    /// Runs `test` again in a process of its own, as the host's part `part`, and reads its
    /// announcement. `test` is the full libtest name, its module first
    /// (`the_engines_group_is_gone_within_a_second_of_its_host::a_test`): `--exact` matches
    /// nothing else in the one test binary (#3613).
    pub fn start(test: &str, part: &str) -> Self {
        Self::start_with(test, part, &[])
    }

    /// [`HostPart::start`], with more environment for the host's part.
    pub fn start_with(test: &str, part: &str, env: &[(&str, &str)]) -> Self {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", test, "--nocapture", "--test-threads=1"])
            .env(PART, part)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (name, value) in env {
            command.env(name, value);
        }
        let mut child = command.spawn().unwrap();
        // What the host's part says on stderr, kept to say why it ended if it does.
        let said = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let mut stderr = child.stderr.take().unwrap();
        let keeping = said.clone();
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stderr.read_to_string(&mut text);
            keeping.lock().unwrap().push_str(&text);
        });
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let started = Instant::now();
        let announcement = loop {
            match lines.next() {
                // libtest may print the test's name ahead of it on the same line.
                Some(Ok(line)) if line.contains("host-part ") => break line,
                Some(Ok(_)) => {}
                other => {
                    let _ = child.wait();
                    std::thread::sleep(Duration::from_millis(200));
                    panic!(
                        "the host's part ended before it said what it started: {other:?}\n{}",
                        said.lock().unwrap()
                    );
                }
            }
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "the host's part said nothing"
            );
        };
        // Keep reading, so the host's part never blocks on a full stdout.
        std::thread::spawn(move || for _ in lines {});
        let at = announcement.find("host-part ").unwrap();
        let mut words = announcement[at..].split_whitespace().skip(1);
        let group = words.next().unwrap().parse().unwrap();
        let member = words.next().unwrap().parse().unwrap();
        let others = words.map(|w| w.parse().unwrap()).collect();
        Self {
            child,
            group,
            member,
            others,
            armed: true,
        }
    }

    /// Kills the host outright, as a crash or `kill -9` would, and reaps it.
    pub fn kill(mut self) {
        self.armed = false;
        self.child.kill().unwrap();
        self.child.wait().unwrap();
    }

    /// Waits for the host to end by itself.
    pub fn wait_for_its_end(mut self) {
        self.armed = false;
        let status = self.child.wait().unwrap();
        assert!(status.success(), "the host's part failed: {status}");
    }

    /// Ends the host by closing its stdin, which it reads as the word to end politely.
    pub fn end_politely(mut self) {
        self.armed = false;
        drop(self.child.stdin.take());
        self.child.wait().unwrap();
    }
}

impl Drop for HostPart {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if self.armed && self.group > 1 {
            let _ = killpg(Pid::from_raw(self.group), Signal::SIGKILL);
        }
    }
}

/// The host's part's one line: the group, its member, and any more processes of its own.
pub fn announce(group: i32, member: u32) {
    announce_all(group, member, &[]);
}

/// [`announce`], with more processes that must go with the group.
pub fn announce_all(group: i32, member: u32, others: &[u32]) {
    let others: Vec<String> = others.iter().map(u32::to_string).collect();
    println!("host-part {group} {member} {}", others.join(" "));
}

/// The host's part waits here until its stdin closes; it is usually killed first.
pub fn wait_to_be_ended() {
    let mut sink = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut sink);
}

/// Stops `pid` as a hung process would be: it no longer runs, and answers nothing.
pub fn stop(pid: u32) {
    kill(Pid::from_raw(pid as i32), Signal::SIGSTOP).unwrap();
}

/// Fails unless the process group `group` and every process in `pids` are gone within
/// `deadline`. Ends whatever is left either way, so a failure leaves nothing running.
pub fn gone_within(group: i32, pids: &[u32], deadline: Duration) {
    let started = Instant::now();
    loop {
        let group_gone = killpg(Pid::from_raw(group), None) == Err(Errno::ESRCH);
        let left: Vec<u32> = pids
            .iter()
            .copied()
            .filter(|&pid| kill(Pid::from_raw(pid as i32), None) != Err(Errno::ESRCH))
            .collect();
        if group_gone && left.is_empty() {
            return;
        }
        if started.elapsed() > deadline {
            let _ = killpg(Pid::from_raw(group), Signal::SIGKILL);
            for pid in &left {
                let _ = kill(Pid::from_raw(*pid as i32), Signal::SIGKILL);
            }
            panic!(
                "after {:?}, group {group} {} and these processes were still there: {left:?}",
                started.elapsed(),
                if group_gone { "was gone" } else { "was not" },
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
