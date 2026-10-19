// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Whether this machine is short on memory: the one reader a dispatch's last step asks
//! (#1467, spec #1434 decision 9, "new starts wait while the machine is short on memory").
//!
//! **The operating system's own verdict, where it gives one**, so no threshold here needs a
//! measurement on a machine under pressure:
//!
//! - **macOS**: the kernel's memory pressure level, `kern.memorystatus_vm_pressure_level`,
//!   read by running `/usr/sbin/sysctl -n` (a `sysctlbyname` call would need `unsafe`), by
//!   its full path, with no shell and no environment, for at most two seconds. It is
//!   `1` while memory is normal, `2` while the system warns and `4` when it is critical. Only
//!   critical is short (D-1467-1): a Mac sits at the warning level for hours under ordinary
//!   heavy use while memory is still being compressed, and holding every dispatch for that
//!   long would hold them for nothing; critical is where the system starts ending programs to
//!   free memory, and one more chat makes that worse.
//! - **Linux**: the kernel's pressure stall information, `/proc/pressure/memory`. Short where
//!   every task that wanted to run was stalled on memory for at least a tenth of the last ten
//!   seconds (`full avg10` at or over [`PSI_FULL_SHORT`], D-1467-2). A kernel without it is
//!   read from `/proc/meminfo`: short where the memory available is under
//!   [`AVAILABLE_FLOOR`] and under [`AVAILABLE_SHARE`] of the whole, the smaller of the two.
//! - **Anything else, or a read that fails**, is [`Memory::Unread`], which holds nothing
//!   (D-1467-3): this is a courtesy to the machine and not a boundary, and a reader that
//!   cannot answer must not stop every dispatch.
//!
//! **A seam a test sets** ([`Gauge::stand_in`]): the app holds one [`Gauge`] per project, and a
//! test puts a reading in place of the machine's without touching the machine.

use std::sync::{Mutex, PoisonError};

/// What this machine's memory is, for a start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Memory {
    /// Short: a new chat waits.
    Short,
    /// Not short.
    Enough,
    /// The machine could not be read, or purlis has no reader for it here. Holds nothing.
    Unread,
}

impl Memory {
    /// Whether a start waits on it.
    pub fn is_short(self) -> bool {
        self == Self::Short
    }
}

/// The pressure stall share, in percent of the last ten seconds, at or over which Linux is
/// short (`full avg10`).
pub const PSI_FULL_SHORT: f64 = 10.0;

/// The memory available, in bytes, under which a Linux without pressure stall information is
/// short, where that is less than [`AVAILABLE_SHARE`] of the whole.
pub const AVAILABLE_FLOOR: u64 = 512 * 1024 * 1024;

/// The share of the whole memory, in percent, under which a Linux without pressure stall
/// information is short, where that is less than [`AVAILABLE_FLOOR`].
pub const AVAILABLE_SHARE: u64 = 5;

/// This machine's memory now, as its operating system says it.
pub fn read() -> Memory {
    #[cfg(target_os = "macos")]
    {
        macos()
    }
    #[cfg(target_os = "linux")]
    {
        linux()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Memory::Unread
    }
}

/// **How much memory this machine has**, in bytes, as its operating system says (#2023): on
/// macOS `hw.memsize`, read as [`read`] reads its level; on Linux `MemTotal` of `/proc/meminfo`.
/// `None` where it could not be read, or purlis has no reader for it here.
pub fn total() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        let mut sysctl = std::process::Command::new("/usr/sbin/sysctl");
        sysctl.args(["-n", "hw.memsize"]);
        bounded_output(&mut sysctl, READ_DEADLINE, READ_MOST).and_then(|said| memsize(&said))
    }
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|text| meminfo_total(&text))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

/// What `sysctl -n hw.memsize` printed, in bytes; `None` where it is no number.
pub fn memsize(said: &str) -> Option<u64> {
    said.trim().parse().ok()
}

/// `MemTotal` of `/proc/meminfo`, in bytes; `None` where the file has none.
pub fn meminfo_total(text: &str) -> Option<u64> {
    meminfo_kib(text, "MemTotal")
}

fn meminfo_kib(text: &str, key: &str) -> Option<u64> {
    text.lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix(':'))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse::<u64>().ok())
        .map(|n| n.saturating_mul(1024))
}

#[cfg(target_os = "macos")]
fn macos() -> Memory {
    let mut sysctl = std::process::Command::new("/usr/sbin/sysctl");
    sysctl.args(["-n", "kern.memorystatus_vm_pressure_level"]);
    bounded_output(&mut sysctl, READ_DEADLINE, READ_MOST)
        .map_or(Memory::Unread, |said| macos_level(&said))
}

/// How long the machine's reader may take: it is asked on a dispatch's own path.
#[cfg(any(target_os = "macos", test))]
const READ_DEADLINE: std::time::Duration = std::time::Duration::from_secs(2);

/// The most of the reader's output read: a level is one number.
#[cfg(any(target_os = "macos", test))]
const READ_MOST: u64 = 64;

/// What `command` printed, at most `most` bytes, where it exits successfully within
/// `deadline`; `None` otherwise, and a command still running at the deadline is killed.
///
/// Run with no environment, no input and its errors dropped, so a reader asked on a dispatch's
/// path can neither hang it nor take anything from purlis's own environment.
#[cfg(any(target_os = "macos", test))]
fn bounded_output(
    command: &mut std::process::Command,
    deadline: std::time::Duration,
    most: u64,
) -> Option<String> {
    use std::io::Read;
    use std::process::Stdio;
    let mut child = crate::forklock::spawn(
        command
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null()),
    )
    .ok()?;
    let until = std::time::Instant::now() + deadline;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                let mut said = String::new();
                child
                    .stdout
                    .take()?
                    .take(most)
                    .read_to_string(&mut said)
                    .ok()?;
                return Some(said);
            }
            Ok(None) if std::time::Instant::now() < until => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Ok(Some(_)) => return None,
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn linux() -> Memory {
    match std::fs::read_to_string("/proc/pressure/memory") {
        Ok(text) => psi(&text),
        Err(_) => {
            std::fs::read_to_string("/proc/meminfo").map_or(Memory::Unread, |text| meminfo(&text))
        }
    }
}

/// What `sysctl -n kern.memorystatus_vm_pressure_level` printed: `4` (critical) or over is
/// short, `1` and `2` are not, anything else is unread.
pub fn macos_level(said: &str) -> Memory {
    match said.trim().parse::<u32>() {
        Ok(level) if level >= 4 => Memory::Short,
        Ok(1..=3) => Memory::Enough,
        _ => Memory::Unread,
    }
}

/// What `/proc/pressure/memory` holds: short where its `full` line's `avg10` is at or over
/// [`PSI_FULL_SHORT`]. A file with no such line is unread.
pub fn psi(text: &str) -> Memory {
    let full = text
        .lines()
        .find_map(|line| line.strip_prefix("full "))
        .and_then(|line| {
            line.split_whitespace()
                .find_map(|field| field.strip_prefix("avg10="))
        })
        .and_then(|avg| avg.parse::<f64>().ok());
    match full {
        Some(avg) if avg >= PSI_FULL_SHORT => Memory::Short,
        Some(_) => Memory::Enough,
        None => Memory::Unread,
    }
}

/// What `/proc/meminfo` holds: short where `MemAvailable` is under the smaller of
/// [`AVAILABLE_FLOOR`] and [`AVAILABLE_SHARE`] of `MemTotal`. A file without both is unread.
pub fn meminfo(text: &str) -> Memory {
    let kib = |key: &str| meminfo_kib(text, key);
    let (Some(total), Some(available)) = (kib("MemTotal"), kib("MemAvailable")) else {
        return Memory::Unread;
    };
    let floor = AVAILABLE_FLOOR.min(total / 100 * AVAILABLE_SHARE);
    if available < floor {
        Memory::Short
    } else {
        Memory::Enough
    }
}

/// **The reader the app asks**, with the seam a test sets: a reading put in place of the
/// machine's ([`Self::stand_in`]) answers until it is taken away.
#[derive(Debug, Default)]
pub struct Gauge {
    stand_in: Mutex<Option<Memory>>,
}

impl Gauge {
    /// The memory a start is judged against now: the stand-in where one is set, else the
    /// machine's ([`read`]).
    pub fn read(&self) -> Memory {
        let stand_in = *self.stand_in.lock().unwrap_or_else(PoisonError::into_inner);
        stand_in.unwrap_or_else(read)
    }

    /// Whether a reading stands in for the machine's: a test's, which then asks for itself
    /// what a timer would otherwise ask.
    pub fn stood_in(&self) -> bool {
        self.stand_in
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
    }

    /// Puts `memory` in place of the machine's reading, or with `None` takes it away.
    pub fn stand_in(&self, memory: Option<Memory>) {
        *self.stand_in.lock().unwrap_or_else(PoisonError::into_inner) = memory;
    }
}

#[cfg(test)]
mod tests;
