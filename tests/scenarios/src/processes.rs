// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Planted processes** (T1.1-22, #6869): the "saw nothing" of a process escape. A batch names
//! the processes an attack aims at (`processes`, `$p1` in a command); the runner starts each one
//! before the chat does, from outside the wrap, and after every step counts the ones an attack
//! reached (`processes.changed == N`). An escape cell says `== 0`, so a refusal is read from the
//! process and never from what the command printed.
//!
//! A process is **changed** when anything a signal could do to it shows: it has ended, it is
//! stopped, or its state cannot be read at all (fail closed: a state the runner cannot name is
//! never "untouched"). Each is a `sleep` that outlives any step, and is killed when the run ends.

use std::collections::BTreeMap;
use std::process::{Child, Command, Stdio};

use crate::play::Malformed;

/// A batch's processes, by key (`p1`), each with what it stands for.
pub type Plan = BTreeMap<String, String>;

/// How long a planted process sleeps: longer than any run.
const SLEEPS_FOR: &str = "3600";

/// **The processes a run planted**, ended when it is dropped.
pub struct Planted {
    running: BTreeMap<String, Child>,
}

impl Planted {
    /// Starts one `sleep` for each of `plan`. A process that cannot start is an error, never a
    /// plan with fewer.
    pub fn start(plan: &Plan) -> Result<Planted, String> {
        let mut running = BTreeMap::new();
        for key in plan.keys() {
            let child = Command::new("sleep")
                .arg(SLEEPS_FOR)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| format!("process `{key}` did not start: {e}"))?;
            running.insert(key.clone(), child);
        }
        Ok(Planted { running })
    }

    /// Each process's id, by key: what `$pN` is bound to.
    pub fn pids(&self) -> BTreeMap<String, u32> {
        self.running
            .iter()
            .map(|(key, child)| (key.clone(), child.id()))
            .collect()
    }

    /// **How many processes an attack has reached**: ended, stopped, or unreadable.
    pub fn changed(&mut self) -> u64 {
        self.changed_by(&stopped)
    }

    /// [`Self::changed`], reading whether a live process is stopped with `stopped`.
    fn changed_by(&mut self, stopped: &dyn Fn(u32) -> Result<bool, String>) -> u64 {
        self.running
            .values_mut()
            .filter_map(|child| is_changed(child, stopped).then_some(()))
            .count() as u64
    }
}

impl Drop for Planted {
    fn drop(&mut self) {
        for child in self.running.values_mut() {
            // A stopped process takes SIGKILL; `wait` keeps no zombie.
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn is_changed(child: &mut Child, stopped: &dyn Fn(u32) -> Result<bool, String>) -> bool {
    match child.try_wait() {
        Ok(None) => stopped(child.id()).unwrap_or(true),
        // It ended, or it cannot be asked.
        Ok(Some(_)) | Err(_) => true,
    }
}

/// Whether process `pid` is stopped (`T` in `ps`'s state), or why it could not be read.
fn stopped(pid: u32) -> Result<bool, String> {
    let out = Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .map_err(|e| format!("ps did not run: {e}"))?;
    if !out.status.success() {
        return Err(format!("ps said {}", out.status));
    }
    let state = String::from_utf8_lossy(&out.stdout);
    let state = state.trim();
    if state.is_empty() {
        return Err("ps said nothing of the process".to_owned());
    }
    Ok(state.starts_with('T'))
}

/// **A command with its `$pN` bound** to the pids, longest key first so `$p10` is never `$p1`
/// and a zero.
pub fn bound(command: &str, pids: &BTreeMap<String, u32>) -> String {
    let mut keys: Vec<_> = pids.iter().collect();
    keys.sort_by_key(|(key, _)| std::cmp::Reverse(key.len()));
    let mut out = command.to_owned();
    for (key, pid) in keys {
        out = out.replace(&format!("${key}"), &pid.to_string());
    }
    out
}

/// **The `$pN` a command names that the batch does not plant**, each as a [`Malformed`]: an
/// unbound name would reach the shell as an empty string, and `kill` of nothing is a refusal that
/// no wrap made.
pub fn unbound(command: &str, plan: &Plan) -> Vec<Malformed> {
    let mut found = Vec::new();
    let bytes = command.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'$' && bytes[i + 1] == b'p' {
            let digits: String = command[i + 2..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            if !digits.is_empty() {
                let key = format!("p{digits}");
                if !plan.contains_key(&key) && !found.iter().any(|m: &Malformed| m.0.contains(&key))
                {
                    found.push(Malformed(format!(
                        "`${key}` is no process of the batch's `processes`"
                    )));
                }
                i += 2 + digits.len();
                continue;
            }
        }
        i += 1;
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one() -> Plan {
        [("p1".to_owned(), "a process outside the chat".to_owned())].into()
    }

    fn signal(pid: u32, name: &str) {
        let status = Command::new("kill")
            .args([&format!("-{name}"), &pid.to_string()])
            .status()
            .expect("kill runs");
        assert!(status.success(), "kill -{name} {pid}");
    }

    /// Whether `ps` can read a process in this run. A sandboxed chat sees no other process, so its
    /// tests of the real reader return with a reason; on CI it is a failure, never a skip.
    fn ps_works() -> bool {
        let mut me = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("sleep starts");
        let works = stopped(me.id()).is_ok();
        let _ = me.kill();
        let _ = me.wait();
        if !works {
            assert!(
                std::env::var_os("CI").is_none() && std::env::var_os("GITHUB_ACTIONS").is_none(),
                "ps cannot read a process on CI: the escape cells would judge nothing"
            );
            eprintln!("skipped: ps cannot read a process here (a sandboxed chat sees no other)");
        }
        works
    }

    fn until(planted: &mut Planted, want: u64) -> u64 {
        let mut seen = u64::MAX;
        for _ in 0..50 {
            seen = planted.changed();
            if seen == want {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        seen
    }

    #[test]
    fn a_process_nobody_touched_is_not_changed() {
        let mut planted = Planted::start(&one()).expect("starts");
        assert_eq!(planted.changed_by(&|_| Ok(false)), 0);
        assert_eq!(planted.pids().len(), 1);
    }

    #[test]
    fn a_process_whose_state_cannot_be_read_is_changed() {
        let mut planted = Planted::start(&one()).expect("starts");
        assert_eq!(planted.changed_by(&|_| Err("no ps".to_owned())), 1);
    }

    #[test]
    fn a_stopped_process_is_changed() {
        let mut planted = Planted::start(&one()).expect("starts");
        assert_eq!(planted.changed_by(&|_| Ok(true)), 1);
    }

    #[test]
    fn a_killed_process_is_changed_whatever_ps_says() {
        let mut planted = Planted::start(&one()).expect("starts");
        signal(planted.pids()["p1"], "KILL");
        let mut seen = 0;
        for _ in 0..50 {
            seen = planted.changed_by(&|_| Ok(false));
            if seen == 1 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(seen, 1);
    }

    #[test]
    fn the_real_reader_sees_a_process_stopped_and_continued() {
        if !ps_works() {
            return;
        }
        let mut planted = Planted::start(&one()).expect("starts");
        let pid = planted.pids()["p1"];
        assert_eq!(planted.changed(), 0);
        signal(pid, "STOP");
        assert_eq!(until(&mut planted, 1), 1, "a stopped process is changed");
        signal(pid, "CONT");
        assert_eq!(
            until(&mut planted, 0),
            0,
            "a continued process is as it was"
        );
    }

    #[test]
    fn a_process_that_cannot_be_asked_about_is_changed() {
        // `stopped` of a pid nothing has: ps says nothing, and nothing is never "untouched".
        assert!(stopped(u32::MAX - 1).is_err());
    }

    #[test]
    fn dropping_the_run_ends_every_process_it_planted() {
        let planted = Planted::start(&one()).expect("starts");
        let pid = planted.pids()["p1"];
        drop(planted);
        let status = Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .expect("kill runs");
        assert!(!status.success(), "the process is gone");
    }

    #[test]
    fn a_command_is_bound_longest_key_first() {
        let pids: BTreeMap<String, u32> = [("p1".to_owned(), 11), ("p10".to_owned(), 1010)].into();
        assert_eq!(bound("kill $p1 $p10", &pids), "kill 11 1010");
    }

    #[test]
    fn a_process_the_batch_does_not_plant_is_malformed() {
        let plan = one();
        assert!(unbound("kill -9 $p1", &plan).is_empty());
        let found = unbound("kill -9 $p2; kill $p1; echo $pwd $p", &plan);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].0.contains("$p2"), "{found:?}");
    }
}
