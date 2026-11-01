// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's terminals**: the one it started on, and any whose other end a process of the chat
//! holds. A program reopens its terminal by name (libuv does, for every Node harness), and a
//! program that makes a terminal opens that one by name too, so both stay open to it. Every
//! other terminal of the machine belongs to something else.

use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

use super::walk::World;

/// The devices a wrapped chat started on, read once as the wrap starts.
pub(super) struct OwnTerminals(Vec<u64>);

impl OwnTerminals {
    /// The terminals this process has as its standard streams and as its controlling terminal.
    /// The chat's side is this process's child, on the same ones.
    pub(super) fn of_this_process() -> OwnTerminals {
        let mut devices: Vec<u64> = [0, 1, 2]
            .iter()
            .filter_map(|fd| std::fs::metadata(format!("/proc/self/fd/{fd}")).ok())
            .filter(|it| it.file_type().is_char_device())
            .map(|it| it.rdev())
            .collect();
        devices.extend(
            std::fs::read_to_string("/proc/self/stat")
                .ok()
                .as_deref()
                .and_then(controlling_terminal),
        );
        OwnTerminals(devices)
    }

    /// Whether the terminal at `path` is one this chat started on.
    fn holds(&self, path: &Path) -> bool {
        std::fs::metadata(path).is_ok_and(|it| self.0.contains(&it.rdev()))
    }

    /// Whether a process of the chat may use `path`: anything that is not a terminal's numbered
    /// name, a terminal it started on, or one whose other end a process inside it holds.
    pub(super) fn allow(&self, world: &World, path: &Path) -> bool {
        let Some(number) = terminal_number(path) else {
            return !names_a_terminal(path);
        };
        self.holds(path) || held_inside(world, number)
    }
}

/// Whether `path` lies directly in the folder of terminals, under a name that is not their
/// multiplexer's: a terminal gone is named with " (deleted)" after its number.
fn names_a_terminal(path: &Path) -> bool {
    path.parent() == Some(Path::new("/dev/pts"))
        && path.file_name().is_some_and(|name| name != "ptmx")
}

/// The number of the terminal `path` names, if it does.
fn terminal_number(path: &Path) -> Option<u32> {
    if !names_a_terminal(path) {
        return None;
    }
    path.file_name()?.to_str()?.parse().ok()
}

/// The controlling terminal's device in a `/proc/<pid>/stat`, `None` where there is none.
fn controlling_terminal(stat: &str) -> Option<u64> {
    // After the name, which may hold anything, and its closing bracket: state, parent, group,
    // session, then the terminal.
    let (_, after) = stat.rsplit_once(')')?;
    let encoded: u64 = after.split_whitespace().nth(4)?.parse().ok()?;
    if encoded == 0 {
        return None;
    }
    let major = (encoded >> 8) & 0xfff;
    let minor = (encoded & 0xff) | ((encoded >> 12) & 0xfff00);
    Some(nix::sys::stat::makedev(major, minor))
}

/// The terminal number in a descriptor's `/proc/<pid>/fdinfo`: only a terminal's other end says.
fn terminal_index(fdinfo: &str) -> Option<u32> {
    fdinfo
        .lines()
        .find_map(|line| line.strip_prefix("tty-index:"))?
        .trim()
        .parse()
        .ok()
}

/// Whether a process inside the sandbox holds the other end of terminal `number`. Read only when
/// a chat opens a terminal it did not start on, which few programs ever do.
fn held_inside(world: &World, number: u32) -> bool {
    let Ok(processes) = std::fs::read_dir("/proc") else {
        return false;
    };
    processes
        .flatten()
        .filter_map(|it| it.file_name().to_str()?.parse::<i32>().ok())
        .any(|pid| world.inside(pid) && holds_other_end(pid, number))
}

/// Whether process `pid` has the other end of terminal `number` open. A process this one may not
/// read holds nothing here.
fn holds_other_end(pid: i32, number: u32) -> bool {
    let Ok(open) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
        return false;
    };
    open.flatten().any(|fd| {
        let multiplexer = std::fs::read_link(fd.path())
            .is_ok_and(|to| to == Path::new("/dev/ptmx") || to == Path::new("/dev/pts/ptmx"));
        multiplexer
            && std::fs::read_to_string(format!(
                "/proc/{pid}/fdinfo/{}",
                fd.file_name().to_string_lossy()
            ))
            .ok()
            .as_deref()
            .and_then(terminal_index)
                == Some(number)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_terminal_is_named_by_its_number_alone() {
        assert_eq!(terminal_number(Path::new("/dev/pts/12")), Some(12));
        for not_numbered in [
            "/dev/pts/ptmx",
            "/dev/ptmx",
            "/dev/tty",
            "/dev/pts",
            "/tmp/3",
        ] {
            assert_eq!(terminal_number(Path::new(not_numbered)), None);
        }
        assert!(!names_a_terminal(Path::new("/dev/pts/ptmx")));
        assert!(names_a_terminal(Path::new("/dev/pts/3 (deleted)")));
    }

    #[test]
    fn the_controlling_terminal_is_read_past_any_name() {
        // Major 136, minor 2: `(pts/2)`.
        let tty_nr = (136 << 8) | 2;
        let stat = format!("7 (a) b (c) S 1 7 7 {tty_nr} 7 0");
        assert_eq!(
            controlling_terminal(&stat),
            Some(nix::sys::stat::makedev(136, 2))
        );
        assert_eq!(controlling_terminal("7 (a) S 1 7 7 0 -1 0"), None);
    }

    #[test]
    fn only_a_terminal_s_other_end_gives_an_index() {
        assert_eq!(
            terminal_index("pos:\t0\nflags:\t02\nmnt_id:\t9\ntty-index:\t4\n"),
            Some(4)
        );
        assert_eq!(terminal_index("pos:\t0\nflags:\t02\n"), None);
    }
}
