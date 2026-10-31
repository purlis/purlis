// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a Claude Code chat on a runner reaches of its runner's link key, tried from inside it.**
//!
//! A home is paired as a cluster runner's is ([`runnerlink::make_at_start`], then
//! [`runnerlink::pair_with_key_made_at_start`]), so the machine is a runner, and the profile of a
//! Claude Code chat there is built as a start builds it, at the full sandbox and at the floor
//! ([`sandbox::decide`], then [`sandbox::claude_wrap::profile`]), never written by hand. Then the
//! real wrap applies that profile to a program that tries each thing and says what came of it.
//!
//! - The key in the session host's own folder, and where that folder's other records go, are
//!   refused by name, through a link, and through a path-only descriptor. A file of the same mode
//!   one folder up is read, so the refusal is the profile's and not the file's.
//! - Nothing is left of the pod's hand-over once the runner is paired.
//! - A process outside the sandbox, of the chat's own user, holds a known value in its environment
//!   and its memory. The same tries made outside the sandbox read it, so what is refused inside is
//!   the sandbox's: the process has allowed any process of its user to trace it, past Yama's rule.
//!
//! The wrap is `sandbox-probe`, which is the wrap as `purlis _sandbox` is, and the workspace's test
//! build puts it beside this binary's folder. `PURLIS_SANDBOX_PROBE` names it instead, and then it
//! must be there. A build of this package alone has none, and only the profile is checked.

#![cfg(target_os = "linux")]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use purlis_core::harness::Harness;
use purlis_core::sandbox::{self, Decided, Form, Machine, Os};
use purlis_core::{forklock, machine, runnerlink};
use purlis_session_protocol::hostlink::{LinkKey, Pinned};

/// Each level's name, and the manifest of a project that starts its chats there.
const LEVELS: [(&str, &str); 2] = [
    (
        "on",
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n",
    ),
    ("floor", "schema = 1\n"),
];

/// What the process outside the sandbox holds. A literal: not a credential.
const HELD: &str = "held-by-the-process-outside-the-sandbox";

/// The process outside the sandbox: it holds [`HELD`], from its environment, in its memory, lets
/// any process of its user trace it, says where that memory is, and waits.
const OUTSIDE: &str = r#"
import ctypes, os, sys
held = ctypes.create_string_buffer(os.environ["HELD"].encode())
ctypes.CDLL(None).prctl(0x59616d61, ctypes.c_ulong(-1), ctypes.c_ulong(0), ctypes.c_ulong(0), ctypes.c_ulong(0))
print(os.getpid(), ctypes.addressof(held), flush=True)
sys.stdin.read()
"#;

/// Each way to what a process holds, given its pid, the address, and the value: one line each, the
/// way's name then `READ` where the value came back, `OPENED` where the way opened and the value
/// did not come, `ok` where a call that returns no value worked, or the error's name.
const TRIES: &str = r#"
import ctypes, errno, os, sys
libc = ctypes.CDLL(None, use_errno=True)
pid, addr, held = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3].encode()
def word(code):
    return errno.errorcode.get(code, str(code))
def say(way, what):
    print(way, what, flush=True)
def by_file(way, path, count, at):
    try:
        fd = os.open(path, os.O_RDONLY)
        data = os.pread(fd, count, at)
    except OSError as e:
        return say(way, word(e.errno))
    say(way, "READ" if held in data else "OPENED")
by_file("environ", "/proc/%d/environ" % pid, 1 << 16, 0)
by_file("mem", "/proc/%d/mem" % pid, len(held), addr)
class IoVec(ctypes.Structure):
    _fields_ = [("base", ctypes.c_void_p), ("len", ctypes.c_size_t)]
into = ctypes.create_string_buffer(len(held))
local, remote = IoVec(ctypes.addressof(into), len(held)), IoVec(addr, len(held))
read_vm = libc.process_vm_readv
read_vm.restype = ctypes.c_ssize_t
read_vm.argtypes = [ctypes.c_int, ctypes.POINTER(IoVec), ctypes.c_ulong, ctypes.POINTER(IoVec), ctypes.c_ulong, ctypes.c_ulong]
got = read_vm(pid, ctypes.byref(local), 1, ctypes.byref(remote), 1, 0)
say("process_vm_readv", word(ctypes.get_errno()) if got < 0 else ("READ" if held in into.raw else "OPENED"))
libc.syscall.restype = ctypes.c_long
pidfd = libc.syscall(ctypes.c_long(434), ctypes.c_long(pid), ctypes.c_long(0))
if pidfd < 0:
    say("pidfd_getfd", word(ctypes.get_errno()))
else:
    taken = libc.syscall(ctypes.c_long(438), ctypes.c_long(pidfd), ctypes.c_long(0), ctypes.c_long(0))
    say("pidfd_getfd", word(ctypes.get_errno()) if taken < 0 else "ok")
libc.ptrace.restype = ctypes.c_long
libc.ptrace.argtypes = [ctypes.c_long, ctypes.c_long, ctypes.c_void_p, ctypes.c_void_p]
PTRACE_SEIZE = 0x4206
say("ptrace", "ok" if libc.ptrace(PTRACE_SEIZE, pid, None, None) == 0 else word(ctypes.get_errno()))
"#;

/// What [`TRIES`] says of each way, outside the sandbox.
const OUTSIDE_THE_SANDBOX: [(&str, &str); 5] = [
    ("environ", "READ"),
    ("mem", "READ"),
    ("process_vm_readv", "READ"),
    ("pidfd_getfd", "ok"),
    ("ptrace", "ok"),
];

/// The process this test started and keeps past the sandbox: ended however the test ends.
struct Outside(Child);

impl Drop for Outside {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The system's python3, which starts with an empty environment, or the first on the `PATH`.
fn python3() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::iter::once(PathBuf::from("/usr/bin"))
        .chain(std::env::split_paths(&path))
        .map(|dir| dir.join("python3"))
        .find(|it| it.is_file())
        .expect("python3 is needed for this test's raw system calls")
}

/// `sandbox-probe`, named or beside this build's test binaries.
fn the_wrap() -> Option<PathBuf> {
    if let Some(named) = std::env::var_os("PURLIS_SANDBOX_PROBE") {
        let named = PathBuf::from(named);
        assert!(named.is_file(), "no program at {}", named.display());
        return Some(named);
    }
    let binary = std::env::current_exe().expect("this test's binary");
    let beside = binary.parent()?.parent()?.join("sandbox-probe");
    beside.is_file().then_some(beside)
}

/// The profile a Claude Code chat in the project at `root`, with its temporary folder `tmp`,
/// starts under on `machine`.
fn the_chats_profile(root: &Path, tmp: &Path, machine: &Machine) -> String {
    let decided = sandbox::decide(Harness::ClaudeCode, root, machine, &|_| true, None, None)
        .expect("decided")
        .expect("a sandbox in force");
    let Decided::Sandboxed(applied) = decided else {
        panic!("not sandboxed: {decided:?}");
    };
    let Form::ClaudeWrapped(wrap) = applied.form_in(Some(root)) else {
        panic!("a Claude Code chat on a runner starts inside purlis's wrap");
    };
    sandbox::claude_wrap::profile(&wrap, root, tmp, &[4040], None).expect("a profile")
}

/// A chat's place at one level, under the profile a start gives it.
struct Chat<'a> {
    wrap: &'a Path,
    profile: String,
    cwd: PathBuf,
    tmp: PathBuf,
}

impl Chat<'_> {
    /// `program` and `args`, started by the wrap as a chat's line is: what it printed.
    fn said(&self, program: String, args: Vec<String>) -> String {
        let tmp = self.tmp.display().to_string();
        let mut command = Command::new(self.wrap);
        command
            .args(purlis_linux_sandbox::line(&self.profile, program, args))
            .env_clear()
            .envs(purlis_linux_sandbox::carried::carried([
                ("PATH", "/usr/bin:/bin"),
                ("TMPDIR", tmp.as_str()),
            ]))
            .current_dir(&self.cwd);
        let out = forklock::output(&mut command).expect("the wrap ran");
        let said = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        assert!(
            out.status.success(),
            "the wrap ended {:?}: {said}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        said
    }

    /// What the probe says of `words`, tried inside the chat's sandbox.
    fn tried(&self, words: &[&str]) -> String {
        self.said(
            self.wrap.display().to_string(),
            words.iter().map(|word| (*word).to_owned()).collect(),
        )
    }
}

/// The lines of [`TRIES`], as way and what came of it.
fn each_way(said: &str) -> Vec<(String, String)> {
    said.lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(way, what)| (way.to_owned(), what.to_owned()))
        .collect()
}

#[test]
fn a_runners_chat_reaches_neither_its_link_key_nor_what_holds_it() {
    purlis_core::unsteered!();
    let held_home = tempfile::tempdir().expect("a home");
    let home = held_home.path().canonicalize().expect("a real home");
    let config = home.join(".config");
    fs::create_dir_all(&config).expect("a config home");

    // The pod's first step, then the desktop's pairing, on folders of this test's.
    let pod = tempfile::tempdir().expect("the pod's folder in memory");
    let handoff = pod.path().join(runnerlink::KEY_FILE);
    let published = pod.path().join("termination-log");
    runnerlink::make_at_start(&handoff, &published).expect("the pod's first step");
    let made = fs::read_to_string(&handoff).expect("the key handed over");
    let desktop = Pinned {
        device: "01JA0DESKT0P00000000000000".into(),
        key: LinkKey::generate().expect("a key").public(),
    };
    runnerlink::pair_with_key_made_at_start(&config, &desktop, &handoff).expect("paired");
    let folder = machine::charterd(&config);
    let key = folder.join(runnerlink::KEY_FILE);
    assert_eq!(fs::read_to_string(&key).expect("the key kept"), made);
    assert!(!handoff.exists(), "the pod's copy is taken away");

    let runner = Machine {
        env: purlis_core::secrets::Env::of(&[("PATH", "/usr/bin:/bin")]),
        home: Some(home.clone()),
        os: Os::Linux,
    };
    assert!(runner.is_a_runner(), "a home paired so is a runner's");

    // A file as private as the key, in the config home above it: only the key's folder is held.
    let beside = config.join("a-file-beside-it");
    fs::write(&beside, "beside").expect("a file beside it");
    fs::set_permissions(&beside, fs::Permissions::from_mode(0o600)).expect("its mode");

    let held_projects = tempfile::tempdir().expect("a folder for the projects");
    let project = held_projects.path().canonicalize().expect("a real folder");
    let chats: Vec<(String, PathBuf, PathBuf, String)> = LEVELS
        .iter()
        .map(|(name, manifest)| {
            let cwd = project.join(name).join("project");
            let tmp = project.join(name).join("tmp");
            fs::create_dir_all(cwd.join(".purlis/vaults")).expect("a project");
            fs::create_dir_all(&tmp).expect("a temporary folder");
            fs::write(cwd.join("purlis.toml"), manifest).expect("the manifest");
            fs::write(cwd.join("notes.json"), "{}\n").expect("a file nothing holds");
            let profile = the_chats_profile(&cwd, &tmp, &runner);
            assert!(
                profile.contains(&folder.display().to_string()),
                "{name}: the profile names the session host's folder"
            );
            ((*name).to_owned(), cwd, tmp, profile)
        })
        .collect();

    let Some(wrap) = the_wrap() else {
        let _ = writeln!(
            std::io::stderr(),
            "no sandbox-probe beside this build: only the profiles were checked"
        );
        return;
    };
    assert!(
        purlis_linux_sandbox::abi() >= purlis_linux_sandbox::MIN_ABI,
        "this kernel's Landlock is version {}, and purlis's Linux sandbox needs {}",
        purlis_linux_sandbox::abi(),
        purlis_linux_sandbox::MIN_ABI
    );

    // The process past the sandbox, and what the same tries read of it with none.
    let python = python3();
    let mut outside = Command::new(&python);
    outside
        .args(["-c", OUTSIDE])
        .env_clear()
        .env("HELD", HELD)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let mut outside = Outside(forklock::spawn(&mut outside).expect("the process outside"));
    let mut said = String::new();
    BufReader::new(outside.0.stdout.take().expect("its output"))
        .read_line(&mut said)
        .expect("what it says");
    let (pid, address) = said.trim().split_once(' ').expect("its pid and address");
    let tries_on = |pid: &str, address: &str| {
        vec![
            "-c".to_owned(),
            TRIES.to_owned(),
            pid.to_owned(),
            address.to_owned(),
            HELD.to_owned(),
        ]
    };
    let mut unheld = Command::new(&python);
    unheld.args(tries_on(pid, address)).env_clear();
    let unheld = forklock::output(&mut unheld).expect("the tries ran");
    let unheld = String::from_utf8_lossy(&unheld.stdout).into_owned();
    let expected: Vec<(String, String)> = OUTSIDE_THE_SANDBOX
        .iter()
        .map(|(way, what)| ((*way).to_owned(), (*what).to_owned()))
        .collect();
    assert_eq!(
        each_way(&unheld),
        expected,
        "outside the sandbox each way must reach the process, or a refusal inside says nothing"
    );

    let (key, folder, beside, handoff) = (
        key.display().to_string(),
        folder.display().to_string(),
        beside.display().to_string(),
        handoff.display().to_string(),
    );
    let told = format!("{folder}/{}", purlis_core::forge::mintport::FILE);
    for (name, cwd, tmp, profile) in chats {
        let chat = Chat {
            wrap: &wrap,
            profile,
            cwd: cwd.clone(),
            tmp,
        };
        // What nothing holds is read: the wrap is at work, and these refusals are its own.
        assert_eq!(chat.tried(&["read", "notes.json"]), "ok", "{name}");
        assert_eq!(chat.tried(&["read", &beside]), "ok", "{name}");

        // The key, by every way to it.
        assert_eq!(chat.tried(&["read", &key]), "EACCES", "{name}");
        assert_eq!(chat.tried(&["list", &folder]), "EACCES", "{name}");
        assert_eq!(chat.tried(&["path-then-read", &key]), "EACCES", "{name}");
        // A link is the profile's to refuse or to let be made: what is held is the read through it.
        chat.tried(&["symlink", &key, "to-the-key"]);
        assert_ne!(chat.tried(&["read", "to-the-key"]), "ok", "{name}");
        assert_eq!(chat.tried(&["rename", &key, "taken"]), "EACCES", "{name}");
        assert_eq!(
            chat.tried(&["link", &key, "second-name"]),
            "EACCES",
            "{name}"
        );

        // Where the runner keeps what it was told of the mint port is not written.
        assert_eq!(chat.tried(&["write", &told]), "EACCES", "{name}");
        assert!(!Path::new(&told).exists(), "{name}");

        // Nothing is left of the pod's copy to read.
        assert_eq!(chat.tried(&["read", &handoff]), "ENOENT", "{name}");

        // Nothing of what the process outside holds comes back, by any way.
        let inside = chat.said(python.display().to_string(), tries_on(pid, address));
        let ways = each_way(&inside);
        assert_eq!(ways.len(), OUTSIDE_THE_SANDBOX.len(), "{name}: {inside}");
        for (way, what) in &ways {
            assert!(
                !matches!(what.as_str(), "READ" | "OPENED" | "ok"),
                "{name}: {way} reached the process outside the sandbox: {what}"
            );
        }
        assert!(!cwd.join("taken").exists(), "{name}");
    }
}
