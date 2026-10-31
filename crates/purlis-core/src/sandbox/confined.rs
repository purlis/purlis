// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The profile an extension that declares a studio capability runs in** (ADR 0101 §2, #2777),
//! written in Seatbelt's language for macOS and read by purlis's own wrap on Linux, as a chat's
//! is ([`super::seatbelt::profile`]).
//!
//! Stricter than a chat's where a chat's is open: a chat reads every file, and this program reads
//! only its own folder, what it writes, and the system's program folders, and of every folder
//! above those only the folder itself, never what is in it. A program folder is granted by its
//! program parts alone (`bin`, `lib`, `share` and their kin, [`PROGRAM_PARTS`]), so a package
//! manager's `etc` and `var`, where configuration and state live, are never read. It writes only
//! its state folder and its own temporary folder. It reaches no address but its own proxy's two
//! loopback ports. It starts programs only from what it reads and cannot write, so never from its
//! state folder or its temporary folder. It asks no system service but the log, so it reads no person's preferences,
//! directory entries or file events through one, and it reads no other process's arguments or
//! environment through the system's settings. Then the denial classes, last, as for every chat:
//! no project's vaults are read, and nothing a later program runs is written.

use std::path::{Path, PathBuf};

use super::Os;
use super::seatbelt::{MOST_PROFILE, TOO_LARGE, kernel_strings, planted_rules, string};

/// The system's own folders a confined program reads whole, on macOS.
const SYSTEM_MACOS: [&str; 6] = [
    "/bin",
    "/sbin",
    "/System",
    "/Library/Apple",
    "/private/etc/ssl",
    "/private/var/db/timezone",
];

/// The system's own folders a confined program reads whole, on Linux.
const SYSTEM_LINUX: [&str; 14] = [
    "/bin",
    "/sbin",
    "/lib",
    "/lib32",
    "/lib64",
    "/libx32",
    "/nix/store",
    "/etc/ld.so.cache",
    "/etc/ld.so.conf",
    "/etc/ld.so.conf.d",
    "/etc/alternatives",
    "/etc/ssl",
    "/etc/ca-certificates",
    "/etc/pki",
];

/// The folders that hold programs and their libraries under a prefix, and nothing a person or a
/// service keeps: what a confined program reads of each prefix in [`PREFIXES_MACOS`] and
/// [`PREFIXES_LINUX`].
const PROGRAM_PARTS: [&str; 11] = [
    "bin",
    "sbin",
    "lib",
    "lib32",
    "lib64",
    "libexec",
    "share",
    "include",
    "opt",
    "Cellar",
    "Frameworks",
];

/// The prefixes programs are installed under, on macOS: the system's, Homebrew's on Apple silicon
/// and on Intel, and the Command Line Tools'.
const PREFIXES_MACOS: [&str; 4] = [
    "/usr",
    "/usr/local",
    "/opt/homebrew",
    "/Library/Developer/CommandLineTools/usr",
];

/// The prefixes programs are installed under, on Linux.
const PREFIXES_LINUX: [&str; 3] = ["/usr", "/usr/local", "/home/linuxbrew/.linuxbrew"];

/// The single files every program may read on either system: the null device and the system's
/// sources of randomness, and the files a TLS client and a name lookup read.
const FILES: [&str; 9] = [
    "/dev/null",
    "/dev/zero",
    "/dev/random",
    "/dev/urandom",
    "/dev/dtracehelper",
    "/private/etc/hosts",
    "/private/etc/localtime",
    "/etc/hosts",
    "/etc/localtime",
];

/// Where macOS keeps the data volume, beside the system's own files: every home and every
/// project is under it by a second name, so a confined program reads none of it.
const DATA_VOLUME: &str = "/System/Volumes/Data";

/// The system's links to folders a path may be written through on macOS: each the link alone.
const LINKS_MACOS: [&str; 3] = ["/var", "/tmp", "/etc"];

/// What a confined program is allowed besides files and its proxy: to fork inside its own
/// sandbox, signal and inspect only what is in it, read the system's settings but those that name
/// other processes, and log. No `user-preference-read`, no directory service and no file events:
/// each reads what a person keeps without a file read.
const BASE: &str = "(version 1)
(deny default)
(allow process-fork)
(allow signal (target same-sandbox))
(allow process-info* (target same-sandbox))
(allow sysctl-read)
(deny sysctl-read (sysctl-name-prefix \"kern.proc\"))
(allow mach-lookup (global-name \"com.apple.logd\"))
";

/// Why a profile was not written: a folder it names could not be listed.
const NOT_LISTED: &str = "a folder of the extension's could not be listed";

/// The folders a confined program reads of the system on `os`: its own folders whole, and of
/// each prefix its program parts alone.
fn system_roots(os: Os) -> Vec<PathBuf> {
    let (whole, prefixes): (&[&str], &[&str]) = match os {
        Os::Linux => (&SYSTEM_LINUX, &PREFIXES_LINUX),
        _ => (&SYSTEM_MACOS, &PREFIXES_MACOS),
    };
    let mut out: Vec<PathBuf> = whole.iter().map(PathBuf::from).collect();
    for prefix in prefixes {
        out.extend(
            PROGRAM_PARTS
                .iter()
                .map(|part| Path::new(prefix).join(part)),
        );
    }
    out
}

/// **Where a program may start from on Linux**: `system` and `reads`, less `writes` and every
/// vault folder or key in `reads`. The kernel's rule names what is beneath a folder and takes
/// nothing back, so a folder that holds one of those is named by what else it holds. A link in it
/// is left out, since the kernel's rule would follow it to what it points at; what it points at
/// is judged when it is started through the link.
fn start_roots(
    system: &[PathBuf],
    reads: &[PathBuf],
    writes: &[PathBuf],
) -> std::io::Result<Vec<PathBuf>> {
    let mut held = writes.to_vec();
    for root in reads {
        vaults_beneath(root, writes, &mut held)?;
    }
    let touches = |root: &Path| {
        held.iter()
            .any(|it| it.starts_with(root) || root.starts_with(it))
    };
    let mut out = Vec::new();
    for root in system {
        let resolved = super::real(root);
        if touches(&resolved) {
            carved(&resolved, &held, &mut out)?;
        } else {
            out.push(root.clone());
        }
    }
    for root in reads {
        carved(root, &held, &mut out)?;
    }
    Ok(out)
}

/// Every vault folder and key beneath `root`, at any depth, into `out`. What it writes is its
/// own, never walked: it is held out whole.
fn vaults_beneath(root: &Path, writes: &[PathBuf], out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in entries(root)? {
        let path = entry.path();
        if writes.contains(&path) {
            continue;
        }
        if super::seatbelt::is_a_vault(&path) {
            out.push(path);
        } else if entry.file_type()?.is_dir() {
            vaults_beneath(&path, writes, out)?;
        }
    }
    Ok(())
}

/// `root` into `out` as what it holds besides `held`: itself where it holds none of them, nothing
/// where it is one or is beneath one.
fn carved(root: &Path, held: &[PathBuf], out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if held.iter().any(|it| root.starts_with(it)) {
        return Ok(());
    }
    if !held.iter().any(|it| it.starts_with(root)) {
        out.push(root.to_path_buf());
        return Ok(());
    }
    for entry in entries(root)? {
        if !entry.file_type()?.is_symlink() {
            carved(&entry.path(), held, out)?;
        }
    }
    Ok(())
}

/// What `folder` holds, in name order so a profile is the same each time; nothing where it is not
/// there.
fn entries(folder: &Path) -> std::io::Result<Vec<std::fs::DirEntry>> {
    let listed = match std::fs::read_dir(folder) {
        Ok(listed) => listed,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(why) => return Err(why),
    };
    let mut out = listed.collect::<std::io::Result<Vec<_>>>()?;
    out.sort_by_key(std::fs::DirEntry::file_name);
    Ok(out)
}

/// **The profile for an extension's program on `os`**, reading `reads` (its own folder) and
/// writing `writes` (its state folder and its temporary folder), reaching the loopback ports
/// `proxy_ports` alone.
pub fn profile(
    os: Os,
    reads: &[PathBuf],
    writes: &[PathBuf],
    proxy_ports: &[u16],
) -> Result<String, &'static str> {
    let mut out = String::from(BASE);
    let mut line = |text: String| {
        out.push_str(&text);
        out.push('\n');
    };
    let system = system_roots(os);
    let mut own: Vec<PathBuf> = reads.to_vec();
    own.extend(writes.iter().cloned());
    let mut read_roots = system.clone();
    read_roots.extend(own.iter().cloned());
    // On macOS a program's libraries are mapped executable from where it reads them, an
    // operation of its own that Linux's sandbox has no word for, and a program starts from there
    // too. Linux's reader is told the same folders in a rule of their own, below.
    let ops = if os == Os::MacOs {
        "file-read* file-map-executable process-exec"
    } else {
        "file-read*"
    };
    line(format!("(allow {ops}"));
    for root in &system {
        for name in kernel_strings(root)? {
            line(format!("  (subpath {name})"));
        }
    }
    for file in FILES {
        line(format!("  (literal {})", string(file)?));
    }
    // On macOS a program's start lists the root folder (measured: it aborts without it), which
    // names the top folders and holds nothing of anybody's.
    if os == Os::MacOs {
        line("  (literal \"/\")".to_owned());
    }
    line(")".to_owned());
    // **The data volume is not the system's**: on macOS `/System/Volumes/Data` holds every
    // person's home and every project under another name, so it is held out of `/System`, and
    // only the program's own folders and a program folder's own second name are given back.
    if os == Os::MacOs {
        line(format!("(deny {ops} (subpath {}))", string(DATA_VOLUME)?));
    }
    line("(allow file-read*".to_owned());
    for root in writes {
        for name in kernel_strings(root)? {
            line(format!("  (subpath {name})"));
        }
    }
    line(")".to_owned());
    line(format!("(allow {ops}"));
    for root in reads {
        for name in kernel_strings(root)? {
            line(format!("  (subpath {name})"));
        }
    }
    for root in system.iter().filter(|root| !root.starts_with("/System")) {
        for name in super::kernel_names(root) {
            if name.starts_with(DATA_VOLUME) {
                line(format!(
                    "  (subpath {})",
                    string(&name.display().to_string())?
                ));
            }
        }
    }
    line(")".to_owned());
    // Linux holds this with the kernel's own rule, which names folders and files by what they
    // are beneath.
    if os == Os::Linux {
        line("(allow process-exec".to_owned());
        for root in &start_roots(&system, reads, writes).map_err(|_| NOT_LISTED)? {
            for name in kernel_strings(root)? {
                line(format!("  (subpath {name})"));
            }
        }
        line(")".to_owned());
    }
    // Of each folder above what it reads, the folder alone: what a path resolves through, never
    // what else a folder holds.
    let mut above = std::collections::BTreeSet::new();
    if os == Os::MacOs {
        above.extend(LINKS_MACOS.iter().map(PathBuf::from));
    }
    for root in &read_roots {
        for name in super::kernel_names(root) {
            for ancestor in name.ancestors().skip(1) {
                above.insert(ancestor.to_path_buf());
            }
        }
    }
    line("(allow file-read-metadata".to_owned());
    for folder in &above {
        line(format!(
            "  (literal {})",
            string(&folder.display().to_string())?
        ));
    }
    line(")".to_owned());
    line("(allow file-write*".to_owned());
    for root in writes {
        for name in kernel_strings(root)? {
            line(format!("  (subpath {name})"));
        }
    }
    line("  (literal \"/dev/null\")".to_owned());
    line(")".to_owned());
    // Its proxy's ports on the two loopback addresses the proxy holds. Seatbelt names them as
    // `localhost`, which on macOS is those two. Linux's reader is told them exactly, since a
    // Linux loopback is all of 127/8, and any other address on it could be another listener.
    for port in proxy_ports {
        if os == Os::Linux {
            line(format!(
                "(allow network-outbound (remote ip \"127.0.0.1:{port}\"))"
            ));
            line(format!(
                "(allow network-outbound (remote ip \"[::1]:{port}\"))"
            ));
        } else {
            line(format!(
                "(allow network-outbound (remote ip \"localhost:{port}\"))"
            ));
        }
    }
    // The denial classes, after every grant: Seatbelt takes the last rule that matches.
    for root in writes {
        for rule in planted_rules(root)? {
            line(rule);
        }
    }
    for rule in super::seatbelt::vaults_everywhere_rules()? {
        line(rule);
    }
    if os == Os::MacOs {
        // What it writes may be beneath what it reads: its state folder is in its own folder.
        for root in writes {
            for name in kernel_strings(root)? {
                line(format!(
                    "(deny file-map-executable process-exec (subpath {name}))"
                ));
            }
        }
        for rule in super::seatbelt::vaults_start_nothing_rules()? {
            line(rule);
        }
    }
    if out.len() > MOST_PROFILE {
        return Err(TOO_LARGE);
    }
    Ok(out)
}
