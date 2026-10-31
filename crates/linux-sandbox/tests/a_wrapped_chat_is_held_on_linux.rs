// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a chat inside purlis's Linux sandbox can do, tried from inside it** (#1040).
//!
//! Each test starts `sandbox-probe` under the wrap with a profile of the shape purlis compiles
//! (a project with a vault, the chat's folder and its temp folder written, the later-code names
//! held at any depth, the proxy's port and the hook socket the only places reached), has it try
//! one thing, and reads what happened. Every denial class is tried, with the ways round each:
//! a link, `..`, a folder moved into place, a process the chat starts, and the chat's own
//! seccomp listener.
//!
//! These run wherever Linux does, and never skip: a kernel without Landlock ABI 6 fails them
//! with a sentence saying so, since the sandbox cannot start there either.

#![cfg(target_os = "linux")]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PROBE: &str = env!("CARGO_BIN_EXE_sandbox-probe");

struct Project {
    _dir: tempfile::TempDir,
    root: PathBuf,
    cwd: PathBuf,
    tmp: PathBuf,
    outside: PathBuf,
    vault: PathBuf,
    proxy: Option<(u16, std::thread::JoinHandle<()>)>,
    hook: PathBuf,
    bus: PathBuf,
    /// The folder a chat serves in and the port it serves on (#2019), as purlis compiles them.
    serve: Option<(PathBuf, u16)>,
}

fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).expect("a real path")
}

/// A project as purlis lays one out, beside a folder outside it, and the profile its chat in
/// `w` starts under.
fn project() -> Project {
    assert!(
        purlis_linux_sandbox::abi() >= purlis_linux_sandbox::MIN_ABI,
        "this kernel's Landlock is version {}, and purlis's Linux sandbox needs {} (Linux 6.12 or \
         later): its tests cannot run here, and neither can a sandboxed chat",
        purlis_linux_sandbox::abi(),
        purlis_linux_sandbox::MIN_ABI
    );
    let dir = tempfile::tempdir().expect("a folder");
    let base = real(dir.path());
    let root = base.join("project");
    let cwd = root.join("w");
    let tmp = base.join("chat-tmp");
    let outside = base.join("outside");
    for folder in [
        &cwd,
        &tmp,
        &outside,
        &root.join(".purlis/vaults"),
        &cwd.join("repo/.git/hooks"),
    ] {
        std::fs::create_dir_all(folder).expect("a folder");
    }
    let vault = root.join(".purlis/vaults/ops.json");
    std::fs::write(&vault, "{\"token\":\"held\"}").expect("a vault");
    std::fs::write(outside.join("notes.txt"), "outside").expect("a file outside");
    std::fs::write(cwd.join("repo/.git/config"), "[core]\n").expect("a clone's config");
    Project {
        root,
        cwd,
        tmp,
        outside,
        vault,
        proxy: None,
        hook: base.join("hooks.sock"),
        bus: base.join("bus"),
        serve: None,
        _dir: dir,
    }
}

impl Project {
    fn profile(&self, port: u16) -> String {
        let mut out = self.profile_unserved(port);
        if let Some((folder, served)) = &self.serve {
            let socket = folder.join(format!("{served}.sock"));
            let socket = socket.display();
            out.push_str(&format!(
                "(allow network-bind (local unix-socket (path-literal \"{socket}\")))\n\
                 (allow network-inbound (local unix-socket (path-literal \"{socket}\")))\n\
                 (allow file-write-create file-write-unlink (literal \"{socket}\"))\n\
                 (allow network-outbound (remote ip \"localhost:{served}\"))\n"
            ));
        }
        out
    }

    fn profile_unserved(&self, port: u16) -> String {
        let cwd = self.cwd.display();
        let tmp = self.tmp.display();
        let vaults = self.root.join(".purlis/vaults");
        let mut out = format!(
            "(version 1)\n(deny default)\n(allow process-exec)\n(allow file-read*)\n\
             (allow network-outbound (remote ip \"localhost:{port}\"))\n\
             (allow network-outbound (remote unix-socket (path-literal \"{}\")))\n\
             (allow file-write*\n  (subpath \"{cwd}\")\n  (subpath \"{tmp}\")\n  (literal \"/dev/null\"))\n\
             (deny file-read* file-write* (subpath \"{}\"))\n\
             (deny file-write* (literal \"{cwd}\"))\n(deny file-write* (literal \"{tmp}\"))\n\
             (deny file-read* file-write* (literal \"{cwd}/held.key\"))\n",
            self.hook.display(),
            vaults.display()
        );
        for root in [&self.cwd, &self.tmp] {
            let root = root.display().to_string().replace('.', "\\\\.");
            for planted in [
                "\\\\.git$",
                "\\\\.git/config(/.*)?$",
                "\\\\.git/hooks(/.*)?$",
                "\\\\.envrc$",
                "\\\\.vscode(/.*)?$",
            ] {
                out.push_str(&format!(
                    "(deny file-write* (regex \"^{root}/(.*/)?{planted}\"))\n"
                ));
            }
            out.push_str(&format!(
                "(deny file-write-create (require-all (vnode-type DIRECTORY) (regex \"^{root}/(.*/)?\\\\.git/modules/.+\")))\n"
            ));
        }
        out
    }

    /// `probe` run under the wrap in the chat's folder: what it printed.
    fn run(&mut self, probe: &[&str]) -> String {
        let port = self.proxy_port();
        let words = purlis_linux_sandbox::line(
            &self.profile(port),
            PROBE.to_owned(),
            probe.iter().map(|it| (*it).to_owned()).collect(),
        );
        let out = wrap_with(&words, &[("TMPDIR".into(), self.tmp.clone().into())])
            .current_dir(&self.cwd)
            .output()
            .expect("the wrap ran");
        let said = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        assert!(
            out.status.success(),
            "the wrap ended {:?}: {said}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        said
    }

    /// A stand-in proxy on a loopback port that answers what it is sent.
    fn proxy_port(&mut self) -> u16 {
        if let Some((port, _)) = &self.proxy {
            return *port;
        }
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let port = listener.local_addr().expect("its address").port();
        let serving = std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                echo(stream);
            }
        });
        self.proxy = Some((port, serving));
        port
    }

    fn s(&self, path: &Path) -> String {
        path.display().to_string()
    }
}

/// The wrap started with `words`, as purlis starts it: from an empty environment, carrying this
/// test's own for the program.
fn wrap(words: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>) -> Command {
    wrap_with(words, &[])
}

/// [`wrap`], with `more` set in the program's environment after this test's own.
fn wrap_with(
    words: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
    more: &[(std::ffi::OsString, std::ffi::OsString)],
) -> Command {
    let mut command = Command::new(PROBE);
    command
        .args(words)
        .env_clear()
        .envs(purlis_linux_sandbox::carried::carried(
            std::env::vars_os().chain(more.iter().cloned()),
        ));
    command
}

fn echo(mut stream: impl Read + Write) {
    let mut got = [0u8; 2];
    if stream.read_exact(&mut got).is_ok() {
        let _ = stream.write_all(&got);
    }
}

fn unix_echo(at: &Path) {
    let listener = UnixListener::bind(at).expect("a socket");
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            echo(stream);
        }
    });
}

/// A terminal this test holds both ends of.
fn a_terminal() -> (std::fs::File, std::fs::File, String) {
    let pair = nix::pty::openpty(None, None).expect("a terminal");
    let name = nix::unistd::ttyname(&pair.slave)
        .expect("its name")
        .display()
        .to_string();
    (pair.master.into(), pair.slave.into(), name)
}

/// `program` under the wrap with `slave` as its terminal: its standard streams and its
/// controlling terminal, as a chat's are. Its output goes to the terminal, unless `output` is
/// set, when its standard output is a pipe.
fn on_terminal(
    p: &mut Project,
    slave: &std::fs::File,
    program: &str,
    args: &[&str],
    output: bool,
) -> std::process::Child {
    let port = p.proxy_port();
    let words = purlis_linux_sandbox::line(
        &p.profile(port),
        program.to_owned(),
        args.iter().map(|it| (*it).to_owned()).collect(),
    );
    let stream = || Stdio::from(slave.try_clone().expect("the terminal again"));
    Command::new("/usr/bin/setsid")
        .arg("--ctty")
        .arg(PROBE)
        .args(words)
        .env_clear()
        .envs(purlis_linux_sandbox::carried::carried(
            std::env::vars_os().chain([("TMPDIR".into(), p.tmp.clone().into())]),
        ))
        .current_dir(&p.cwd)
        .stdin(stream())
        .stdout(if output { Stdio::piped() } else { stream() })
        .stderr(stream())
        .spawn()
        .expect("the wrap started")
}

/// What the probe printed, tried under the wrap on the terminal `slave`.
fn probed_on_terminal(p: &mut Project, slave: &std::fs::File, probe: &[&str]) -> String {
    let child = on_terminal(p, slave, PROBE, probe, true);
    let out = child.wait_with_output().expect("the wrap ended");
    assert!(out.status.success(), "the wrap ended {:?}", out.status);
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
fn a_chat_uses_its_own_terminal_by_name_and_as_dev_tty() {
    let mut p = project();
    let (_master, slave, _) = a_terminal();
    assert_eq!(probed_on_terminal(&mut p, &slave, &["own-terminal"]), "ok");
}

#[test]
fn a_chat_opens_no_terminal_but_its_own() {
    let mut p = project();
    let (_master, slave, _) = a_terminal();
    let (other_master, _other_slave, other) = a_terminal();
    for probe in ["terminal-write", "terminal-read"] {
        assert_eq!(
            probed_on_terminal(&mut p, &slave, &[probe, &other]),
            "EACCES",
            "{probe} {other}"
        );
    }
    // Nor through a chat with no terminal at all.
    assert_eq!(p.run(&["terminal-write", &other]), "EACCES");
    nix::fcntl::fcntl(
        &other_master,
        nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
    )
    .expect("not waited for");
    let mut got = [0u8; 8];
    assert!(
        (&other_master).read(&mut got).is_err(),
        "nothing reached the other terminal"
    );
}

#[test]
fn a_chat_opens_a_terminal_it_made_by_name() {
    let mut p = project();
    let (_master, slave, _) = a_terminal();
    assert_eq!(
        probed_on_terminal(&mut p, &slave, &["terminal-of-its-own"]),
        "ok"
    );
    assert_eq!(p.run(&["terminal-of-its-own"]), "ok");
}

#[test]
fn an_interactive_shell_and_the_commands_in_it_keep_their_terminal() {
    let mut p = project();
    let (mut master, slave, name) = a_terminal();
    let mut child = on_terminal(&mut p, &slave, "/bin/sh", &["-i"], false);
    drop(slave);
    master
        .write_all(b"stty size; tty; echo done-$((1+1)); exit\n")
        .expect("typed");
    let reader = std::thread::spawn(move || {
        let mut said = Vec::new();
        // The terminal's end reads as an error once the last program on it has gone.
        let _ = master.read_to_end(&mut said);
        String::from_utf8_lossy(&said).into_owned()
    });
    assert!(child.wait().expect("the wrap ended").success());
    let said = reader.join().expect("read");
    assert!(said.contains("done-2"), "{said}");
    assert!(said.contains(&name), "{said}");
    assert!(!said.contains("not a tty"), "{said}");
}

#[test]
fn the_chat_writes_its_own_folder_and_temp_folder_and_nothing_else() {
    let mut p = project();
    assert_eq!(p.run(&["write", "made.txt"]), "ok");
    assert_eq!(
        std::fs::read_to_string(p.cwd.join("made.txt")).expect("made"),
        "x"
    );
    assert_eq!(p.run(&["write", &p.s(&p.tmp.join("t"))]), "ok");
    assert_eq!(p.run(&["mkdir", "sub"]), "ok");
    assert_eq!(p.run(&["write", "sub/deeper.txt"]), "ok");
    assert_eq!(p.run(&["write", "/dev/null"]), "ok");
    for outside in [
        p.root.join("notes.md"),
        p.outside.join("notes.txt"),
        p.outside.join("new.txt"),
        PathBuf::from("/etc/purlis-probe"),
        PathBuf::from("/tmp/purlis-probe"),
        PathBuf::from("/dev/shm/purlis-probe"),
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(".purlis-probe"),
    ] {
        assert_eq!(
            p.run(&["write", &p.s(&outside)]),
            "EACCES",
            "{}",
            outside.display()
        );
    }
    assert_eq!(
        p.run(&["append", &p.s(&p.outside.join("notes.txt"))]),
        "EACCES"
    );
    assert_eq!(
        p.run(&["truncate", &p.s(&p.outside.join("notes.txt"))]),
        "EACCES"
    );
    assert_eq!(
        p.run(&["chmod", &p.s(&p.outside.join("notes.txt")), "777"]),
        "EACCES"
    );
    assert_eq!(
        p.run(&["unlink", &p.s(&p.outside.join("notes.txt"))]),
        "EACCES"
    );
    assert_eq!(p.run(&["mkdir", &p.s(&p.outside.join("d"))]), "EACCES");
    assert_eq!(
        p.run(&["touch", &p.s(&p.outside.join("notes.txt"))]),
        "EACCES"
    );
    assert_eq!(p.run(&["touch", "made.txt"]), "ok");
    assert_eq!(p.run(&["chown", "made.txt"]), "EPERM");
    assert_eq!(
        std::fs::read_to_string(p.outside.join("notes.txt")).expect("still there"),
        "outside"
    );
}

#[test]
fn a_link_or_dot_dot_reaches_nothing_the_chat_could_not_reach_by_name() {
    let mut p = project();
    let out = p.s(&p.outside);
    assert_eq!(p.run(&["symlink", &out, "away"]), "ok");
    assert_eq!(p.run(&["write", "away/through-link.txt"]), "EACCES");
    assert_eq!(p.run(&["mkdir", "away/d"]), "EACCES");
    assert_eq!(
        p.run(&["symlink", &format!("{out}/dangling.txt"), "dangling"]),
        "ok"
    );
    assert_eq!(p.run(&["write", "dangling"]), "EACCES");
    assert!(!p.outside.join("dangling.txt").exists());
    assert_eq!(p.run(&["write", "../../outside/up.txt"]), "EACCES");
    assert_eq!(p.run(&["write", "sub/../../notes.md"]), "ENOENT");
    assert_eq!(p.run(&["mkdir", "sub"]), "ok");
    assert_eq!(p.run(&["write", "sub/../../notes.md"]), "EACCES");
    assert_eq!(p.run(&["rename", "made-later", "../moved"]), "ENOENT");
    assert_eq!(p.run(&["write", "made-later"]), "ok");
    assert_eq!(p.run(&["rename", "made-later", "../moved"]), "EACCES");
    assert_eq!(p.run(&["rename", "made-later", "away/moved"]), "EACCES");
    assert_eq!(
        p.run(&["link", &p.s(&p.outside.join("notes.txt")), "hard"]),
        "EACCES"
    );
    assert_eq!(
        p.run(&[
            "write",
            &format!("/proc/self/root{}", p.outside.join("x").display())
        ]),
        "EACCES"
    );
    assert_eq!(p.run(&["write", "/proc/self/cwd/../escaped"]), "EACCES");
    assert!(!p.outside.join("through-link.txt").exists());
    assert!(!p.outside.join("up.txt").exists());
    assert!(!p.root.join("moved").exists());
}

#[test]
fn a_vault_is_never_read_by_any_way_to_it() {
    let mut p = project();
    let vault = p.s(&p.vault);
    assert_eq!(p.run(&["read", &vault]), "EACCES");
    assert_eq!(
        p.run(&["list", &p.s(&p.root.join(".purlis/vaults"))]),
        "EACCES"
    );
    assert_eq!(p.run(&["read", "../.purlis/vaults/ops.json"]), "EACCES");
    assert_eq!(p.run(&["symlink", &vault, "v"]), "ok");
    assert_eq!(p.run(&["read", "v"]), "EACCES");
    assert_eq!(p.run(&["link", &vault, "hard"]), "EACCES");
    assert_eq!(p.run(&["rename", &vault, "stolen"]), "EACCES");
    assert_eq!(
        p.run(&["read", &format!("/proc/self/root{vault}")]),
        "EACCES"
    );
    assert_eq!(p.run(&["write", &vault]), "EACCES");
    // A path-only descriptor is the kernel's to open, and reading through it is judged again.
    assert_eq!(p.run(&["path-then-read", &vault]), "EACCES");
    assert_eq!(
        p.run(&["path-then-read", &p.s(&p.outside.join("notes.txt"))]),
        "ok"
    );
    assert_eq!(p.run(&["read", &p.s(&p.outside.join("notes.txt"))]), "ok");
}

#[test]
fn what_a_later_program_loads_is_never_written_at_any_depth() {
    let mut p = project();
    assert_eq!(p.run(&["write", "repo/.git/hooks/pre-commit"]), "EACCES");
    assert_eq!(p.run(&["append", "repo/.git/config"]), "EACCES");
    assert_eq!(p.run(&["truncate", "repo/.git/config"]), "EACCES");
    assert_eq!(p.run(&["touch", "repo/.git/config"]), "EACCES");
    assert_eq!(p.run(&["unlink", "repo/.git/config"]), "EACCES");
    assert_eq!(p.run(&["write", ".envrc"]), "EACCES");
    assert_eq!(p.run(&["mkdir", "a"]), "ok");
    assert_eq!(p.run(&["mkdir", "a/.git"]), "EACCES");
    assert_eq!(p.run(&["mkdir", "a/.vscode"]), "EACCES");
    // Built under another name and moved into place, whole.
    assert_eq!(p.run(&["mkdir", "a/hooks-to-be"]), "ok");
    assert_eq!(p.run(&["write", "a/hooks-to-be/post-checkout"]), "ok");
    assert_eq!(
        p.run(&["rename", "a/hooks-to-be", "repo/.git/hooks"]),
        "EACCES"
    );
    assert_eq!(p.run(&["rename", "repo/.git/hooks", "a/taken"]), "EACCES");
    assert_eq!(p.run(&["write", "script"]), "ok");
    assert_eq!(p.run(&["rename", "script", ".envrc"]), "EACCES");
    assert_eq!(
        p.run(&["link", "script", "repo/.git/hooks/pre-push"]),
        "EACCES"
    );
    assert_eq!(p.run(&["symlink", "script", ".envrc"]), "EACCES");
    assert_eq!(
        p.run(&["tmpfile-link", ".", "repo/.git/hooks/post-merge"]),
        "EACCES"
    );
    assert_eq!(p.run(&["tmpfile-link", ".", "fine-name"]), "ok");
    assert_eq!(p.run(&["mkdir", "repo/.git/modules"]), "ok");
    assert_eq!(p.run(&["mkdir", "repo/.git/modules/sub"]), "EACCES");
    assert_eq!(
        p.run(&["write", &p.s(&p.tmp.join("x/.git/config"))]),
        "ENOENT"
    );
    assert_eq!(p.run(&["mkdir", &p.s(&p.tmp.join(".git"))]), "EACCES");
    assert!(!p.cwd.join("repo/.git/hooks/pre-commit").exists());
}

#[test]
fn the_chat_s_own_folders_are_never_moved_or_removed() {
    let mut p = project();
    let cwd = p.s(&p.cwd);
    let tmp = p.s(&p.tmp);
    assert_eq!(p.run(&["rename", &cwd, &format!("{cwd}-moved")]), "EACCES");
    assert_eq!(p.run(&["rename", &tmp, &format!("{cwd}/tmp-in")]), "EACCES");
    assert_eq!(p.run(&["rmdir", &tmp]), "EACCES");
    assert_eq!(p.run(&["chmod", &cwd, "700"]), "EACCES");
}

#[test]
fn only_the_proxy_and_the_hook_socket_are_reached() {
    let mut p = project();
    let port = p.proxy_port().to_string();
    unix_echo(&p.hook);
    unix_echo(&p.bus);
    assert_eq!(p.run(&["tcp", "127.0.0.1", &port]), "ok");
    let other = TcpListener::bind("127.0.0.1:0").expect("another port");
    let other_port = other.local_addr().expect("its address").port().to_string();
    assert_eq!(p.run(&["tcp", "127.0.0.1", &other_port]), "EACCES");
    // Another machine, on the proxy's own port: refused before any packet leaves.
    assert_eq!(p.run(&["tcp", "192.0.2.1", &port]), "EACCES");
    assert_eq!(p.run(&["tcp", "2001:db8::1", &port]), "EACCES");
    assert_eq!(p.run(&["udp", "192.0.2.1", "53"]), "EACCES");
    assert_eq!(p.run(&["udp", "127.0.0.1", &port]), "EACCES");
    assert_eq!(p.run(&["unix", &p.s(&p.hook)]), "ok");
    // Through a link, a socket is judged by what it reaches, as the kernel names it.
    std::os::unix::fs::symlink(&p.hook, p.cwd.join("hook-link")).expect("a link");
    std::os::unix::fs::symlink(&p.bus, p.cwd.join("bus-link")).expect("a link");
    assert_eq!(p.run(&["unix", "hook-link"]), "ok");
    assert_eq!(p.run(&["unix", "bus-link"]), "EACCES");
    // The session bus, where the credential store answers, as any socket but the hook's.
    assert_eq!(p.run(&["unix", &p.s(&p.bus)]), "EACCES");
    assert_eq!(p.run(&["abstract", "purlis-probe"]), "EACCES");
}

/// A socket listens only where it was bound: one the chat never bound is refused rather than
/// bound by the kernel to a port it picks on every interface.
#[test]
fn a_socket_never_bound_does_not_listen() {
    let mut p = project();
    assert_eq!(p.run(&["listen-unbound", "4"]), "EACCES");
    assert_eq!(p.run(&["listen-unbound", "6"]), "EACCES");
}

/// The wrap started on `probe` in the chat's folder, with no port served, telling what it
/// refused to the file it returns.
fn told_command(p: &Project, probe: &[&str]) -> (Command, PathBuf) {
    let told = p.tmp.join("told");
    let _ = std::fs::remove_file(&told);
    let words = purlis_linux_sandbox::line(
        &p.profile_unserved(free_port()),
        PROBE.to_owned(),
        probe.iter().map(|it| (*it).to_owned()).collect(),
    );
    let mut command = wrap_with(
        &words,
        &[
            ("TMPDIR".into(), p.tmp.clone().into()),
            ("PROBE_TOLD".into(), told.clone().into()),
        ],
    );
    command.current_dir(&p.cwd);
    (command, told)
}

/// What the wrap `command` printed and, from `told`, what it told it refused, a line each.
fn said_and_told(out: std::process::Output, told: &Path) -> (String, String) {
    assert!(out.status.success(), "{out:?}");
    (
        String::from_utf8_lossy(&out.stdout).trim().to_owned(),
        std::fs::read_to_string(told).unwrap_or_default(),
    )
}

/// `probe` run under the wrap in the chat's folder, with no port served: what it printed, and
/// what the wrap told it refused, a line each.
fn run_told(p: &Project, probe: &[&str]) -> (String, String) {
    let (mut command, told) = told_command(p, probe);
    said_and_told(command.output().expect("the wrap ran"), &told)
}

/// #3396 (ruled 2026-10-30): a test server on a port the system picks, on 127.0.0.1, serves
/// with no grant, and the chat reaches it there; nothing is told.
#[test]
fn a_test_server_on_a_port_the_system_picks_on_127_0_0_1_serves_and_is_reached() {
    let p = project();
    let (said, told) = run_told(&p, &["serve-and-dial", "127.0.0.1"]);
    assert_eq!(said, "ok", "told: {told:?}");
    assert_eq!(told, "", "nothing refused");
    // Again, as a test suite starts one server after another.
    for _ in 0..3 {
        assert_eq!(run_told(&p, &["serve-and-dial", "127.0.0.1"]).0, "ok");
    }
}

/// #3396: a port the system picks on any other address (every interface, IPv6's loopback) is
/// still refused, and told, so its chat gets the Notice that says what to do: the bind on port
/// 0, as the address it asked for.
#[test]
fn a_bind_on_a_port_the_system_picks_elsewhere_is_refused_and_told() {
    let p = project();
    for (address, as_told) in [
        ("0.0.0.0", "0.0.0.0:0"),
        ("::", "[::]:0"),
        ("::1", "[::1]:0"),
    ] {
        let (said, told) = run_told(&p, &["serve-on", address, "0"]);
        assert_eq!(said, "EACCES", "{address}");
        assert!(
            told.lines()
                .any(|line| line == format!("network-bind {as_told}")),
            "{address} told: {told:?}"
        );
    }
}

/// #3396: a chat with a test server of its own reaches only that server's port: a service on
/// this machine's loopback it did not start is refused, as before.
#[test]
fn a_chat_with_a_server_of_its_own_reaches_no_other_loopback_port() {
    let p = project();
    let other = TcpListener::bind("127.0.0.1:0").expect("another service");
    let other_port = other.local_addr().expect("its address").port().to_string();
    std::thread::spawn(move || {
        for stream in other.incoming().flatten() {
            echo(stream);
        }
    });
    let (said, _) = run_told(&p, &["serve-and-dial", "127.0.0.1", &other_port]);
    assert_eq!(said, "EACCES");
    // A fixed port goes the way it did: refused until a grant serves it (#2019).
    let fixed = free_port().to_string();
    assert_eq!(run_told(&p, &["serve-on", "127.0.0.1", &fixed]).0, "EACCES");
}

/// #3396: a datagram socket on 127.0.0.1 at a port the system picks is still refused: only a
/// TCP socket is given a picked port.
#[test]
fn a_datagram_socket_is_given_no_port_the_system_picks() {
    let p = project();
    let (said, told) = run_told(&p, &["udp-bind"]);
    assert_eq!(said, "EACCES", "told: {told:?}");
}

/// #3396: a test server that stopped is dialled where nothing listens any more, and answered as
/// the kernel answers that, connection refused, with nothing told: no Notice offers to allow a
/// port the system picked.
#[test]
fn a_test_server_that_stopped_is_refused_as_nothing_listening_and_not_told() {
    let p = project();
    let (said, told) = run_told(&p, &["picked-closed-dial"]);
    assert_eq!(said, "ECONNREFUSED", "told: {told:?}");
    assert!(
        !told
            .lines()
            .any(|line| line.starts_with("network-outbound")),
        "told: {told:?}"
    );
}

/// #3396: once a chat's test server stopped, a service outside the chat that listens on the port
/// it had is not the chat's to reach: the port was picked for the chat, but the socket listening
/// there is not the one made for it. Refused, and not told.
#[test]
fn a_service_that_took_a_port_a_test_server_had_is_not_reached() {
    let p = project();
    let file = p.tmp.join("picked-port");
    let (mut command, told) = told_command(&p, &["picked-closed-dial", &p.s(&file)]);
    let child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the wrap started");
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let port = loop {
        if let Some(port) = std::fs::read_to_string(&file)
            .ok()
            .and_then(|it| it.trim().parse::<u16>().ok())
        {
            break port;
        }
        assert!(std::time::Instant::now() < until, "the probe wrote no port");
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let other = TcpListener::bind(("127.0.0.1", port)).expect("the port, taken outside");
    std::thread::spawn(move || {
        for stream in other.incoming().flatten() {
            echo(stream);
        }
    });
    std::fs::write(p.tmp.join("picked-port.go"), "").expect("go");
    let (said, told) = said_and_told(child.wait_with_output().expect("the wrap ended"), &told);
    assert_eq!(said, "EACCES", "told: {told:?}");
    assert!(
        !told
            .lines()
            .any(|line| line.starts_with("network-outbound")),
        "told: {told:?}"
    );
}

/// #3396: Python's own test server, `HTTPServer(("127.0.0.1", 0))`, serves a request its chat
/// makes to it.
#[test]
fn python_s_test_server_on_a_port_the_system_picks_answers() {
    let mut p = project();
    let script = r#"
import http.server, threading, urllib.request
class Answer(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b"ok")
    def log_message(self, *args):
        pass
try:
    server = http.server.HTTPServer(("127.0.0.1", 0), Answer)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    url = "http://127.0.0.1:%d/" % server.server_port
    print(opener.open(url, timeout=10).read().decode(), flush=True)
except Exception as e:
    print(repr(e), flush=True)
"#;
    assert_eq!(python(&mut p, script, &[]), "ok");
}

/// A thread with a descriptor table of its own is judged by its own socket: its unbound one is
/// refused, though the process's table holds a bound socket at the same number.
#[test]
fn a_thread_with_its_own_descriptors_is_judged_by_its_own_socket() {
    let mut p = project();
    assert_eq!(
        p.run(&["listen-unshared", "purlis-probe-unshared"]),
        "EACCES"
    );
}

/// A program a runner's host starts through `_no-namespaces` makes no namespace, whatever its
/// machine lets a program make, and neither does what it starts: on a runner only the browser
/// engine may (#3298, D-2050-25). It still runs as it was asked to.
#[test]
fn a_program_started_without_namespaces_makes_none() {
    // The same probe, not held: this machine lets it make one, so what is refused below is the
    // filter's refusal.
    let out = Command::new(PROBE)
        .arg("user-namespace")
        .output()
        .expect("the probe ran");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "ok",
        "this machine refuses a user namespace to anyone: {out:?}"
    );
    let out = Command::new(PROBE)
        .args(purlis_linux_sandbox::no_namespaces_line(
            PROBE.into(),
            vec!["user-namespace".into()],
        ))
        .output()
        .expect("the probe ran");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "EPERM",
        "{out:?}"
    );
    let out = Command::new(PROBE)
        .args(purlis_linux_sandbox::no_namespaces_line(
            "/bin/sh".into(),
            vec!["-c".into(), format!("{PROBE} user-namespace").into()],
        ))
        .output()
        .expect("the shell ran");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "EPERM",
        "{out:?}"
    );
    let out = Command::new(PROBE)
        .args(purlis_linux_sandbox::no_namespaces_line(
            PROBE.into(),
            vec!["read".into(), "/etc/passwd".into()],
        ))
        .output()
        .expect("the probe ran");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok", "{out:?}");
}

/// A port free on this machine now.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|it| it.local_addr())
        .expect("a free port")
        .port()
}

/// #2019: a chat serves on its own socket in the folder purlis made for it, and nowhere else: a
/// server it starts on its port, at whatever address, is bound to that socket instead, and any
/// other port or path is refused.
#[test]
fn a_chat_serves_on_its_own_socket_whatever_address_it_asks_for() {
    let mut p = project();
    let folder = p.tmp.parent().expect("a parent").join("serve");
    std::fs::create_dir_all(&folder).expect("the chat's folder");
    let port = free_port();
    let socket = folder.join(format!("{port}.sock"));
    let port_word = port.to_string();
    // Nothing served before it is allowed.
    assert_eq!(p.run(&["serve-on", "127.0.0.1", &port_word]), "EACCES");
    p.serve = Some((folder.clone(), port));
    for address in ["127.0.0.1", "0.0.0.0", "::"] {
        assert_eq!(p.run(&["serve-on", address, &port_word]), "ok", "{address}");
        let meta = std::fs::symlink_metadata(&socket).expect("its socket");
        assert!(std::os::unix::fs::FileTypeExt::is_socket(&meta.file_type()));
        assert!(
            TcpStream::connect(("127.0.0.1", port)).is_err(),
            "no TCP port"
        );
    }
    assert_eq!(p.run(&["serve-at", &p.s(&socket)]), "ok");
    assert_eq!(
        p.run(&["serve-on", "127.0.0.1", &(port.wrapping_add(1)).to_string()]),
        "EACCES"
    );
    assert_eq!(
        p.run(&["serve-at", &p.s(&folder.join("other.sock"))]),
        "EACCES"
    );
}

/// A server on an abstract unix socket, which the chat binds itself, listens as before.
#[test]
fn a_server_on_a_socket_the_chat_bound_listens() {
    let mut p = project();
    assert_eq!(p.run(&["abstract-listen", "purlis-probe-listen"]), "ok");
}

/// `script` run by `python3` under the wrap in the chat's folder, with `args`: what it printed.
fn python(p: &mut Project, script: &str, args: &[&str]) -> String {
    let Ok(python) = which("python3") else {
        panic!("python3 is needed for this test's raw system calls");
    };
    let port = p.proxy_port();
    let mut words = vec!["-c".to_owned(), script.to_owned()];
    words.extend(args.iter().map(|it| (*it).to_owned()));
    let out = wrap(purlis_linux_sandbox::line(&p.profile(port), python, words))
        .current_dir(&p.cwd)
        .output()
        .expect("the wrap ran");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// A server a thread starts after the process's first thread has ended listens: the thread's
/// own descriptor table is read, where its process's leader holds none any more.
#[test]
fn a_server_started_after_the_first_thread_ended_listens() {
    let mut p = project();
    let script = r#"
import ctypes, errno, os, platform, socket, sys, threading, time
SYS_EXIT = 60 if platform.machine() == "x86_64" else 93
leader = os.getpid()
def serve():
    stat = "/proc/self/task/%d/stat" % leader
    for _ in range(500):
        try:
            if open(stat).read().rsplit(")", 1)[1].split()[0] in ("Z", "X"):
                break
        except OSError:
            break
        time.sleep(0.01)
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.bind("\0" + sys.argv[1])
    try:
        s.listen(1)
        print("ok", flush=True)
    except OSError as e:
        print(errno.errorcode.get(e.errno, str(e.errno)), flush=True)
    os._exit(0)
threading.Thread(target=serve).start()
ctypes.CDLL(None).syscall(SYS_EXIT, 0)
"#;
    assert_eq!(python(&mut p, script, &["purlis-probe-leader-gone"]), "ok");
}

/// A connected socket is never disconnected (`connect` with `AF_UNSPEC`): the supervised
/// `connect` refuses every family but unix and the internet's. A listen relies on it, since a
/// disconnected socket would be unbound again where the supervisor judged it bound.
#[test]
fn a_connected_socket_is_never_disconnected() {
    let mut p = project();
    let port = p.proxy_port().to_string();
    let script = r#"
import ctypes, errno, os, socket, sys
libc = ctypes.CDLL(None, use_errno=True)
s = socket.create_connection(("127.0.0.1", int(sys.argv[1])))
unspec = ctypes.create_string_buffer(16)
r = libc.connect(s.fileno(), unspec, 16)
print("ok" if r == 0 else errno.errorcode.get(ctypes.get_errno(), "?"), flush=True)
"#;
    assert_eq!(python(&mut p, script, &[&port]), "EACCES");
}

#[test]
fn a_process_the_chat_starts_is_held_the_same_and_cannot_lift_it() {
    let mut p = project();
    let status = p.run(&["status"]);
    assert!(status.starts_with("seccomp=2 "), "{status}");
    assert!(status.ends_with("nnp=1"), "{status}");
    // A shell started inside, and a program it starts in turn.
    let shell = format!(
        "{PROBE} write {o} ; {PROBE} write inner.txt ; {PROBE} status",
        o = p.outside.join("from-shell").display()
    );
    let out = wrap(purlis_linux_sandbox::line(
        &{
            let port = p.proxy_port();
            p.profile(port)
        },
        "/bin/sh".to_owned(),
        vec!["-c".to_owned(), shell],
    ))
    .current_dir(&p.cwd)
    .output()
    .expect("the wrap ran");
    let said = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = said.lines().collect();
    assert_eq!(lines.first().copied(), Some("EACCES"), "{said}");
    assert_eq!(lines.get(1).copied(), Some("ok"), "{said}");
    assert!(
        lines.get(2).is_some_and(|it| it.starts_with("seccomp=2 ")),
        "{said}"
    );
    assert!(!p.outside.join("from-shell").exists());
    // Undumpable, it is still served, and still held.
    assert_eq!(p.run(&["undumpable", "after.txt"]), "ok");
    // Nothing outside the sandbox is signalled.
    assert_eq!(p.run(&["signal-parent"]), "EPERM");
}

/// A profile of the confined shape: programs start from the system's folders and `from` alone,
/// though every file is read, so what is held is the start and never the file.
fn starts_only_from(from: &Path, written: &Path) -> String {
    let mut roots = String::new();
    for root in ["/usr", "/bin", "/sbin", "/lib", "/lib64", "/nix/store"] {
        roots.push_str(&format!(" (subpath \"{root}\")"));
    }
    format!(
        "(version 1)\n(deny default)\n(allow process-exec{roots} (subpath \"{}\"))\n\
         (allow file-read*)\n(allow file-write* (subpath \"{}\") (literal \"/dev/null\"))\n",
        from.display(),
        written.display()
    )
}

/// The status and output of `sh -c script` under `profile`.
fn sh_under(profile: &str, cwd: &Path, script: &str) -> (Option<i32>, String) {
    let out = wrap(purlis_linux_sandbox::line(
        profile,
        "/bin/sh".to_owned(),
        vec!["-c".to_owned(), script.to_owned()],
    ))
    .current_dir(cwd)
    .output()
    .expect("the wrap ran");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).trim().to_owned(),
    )
}

fn probe_copied_to(dir: &Path) -> PathBuf {
    let at = dir.join("copied-probe");
    std::fs::copy(PROBE, &at).expect("a copy of the probe");
    at
}

#[test]
fn a_program_starts_only_from_where_the_profile_says() {
    let p = project();
    let inside = probe_copied_to(&p.cwd);
    let outside = probe_copied_to(&p.outside);
    let profile = starts_only_from(&p.cwd, &p.cwd);
    // The system's shell and its tools, and a compiled program with the system's dynamic linker
    // and libraries, from a folder it may start from.
    let (status, said) = sh_under(
        &profile,
        &p.cwd,
        &format!("/bin/echo tool; {} status", inside.display()),
    );
    assert_eq!(status, Some(0), "{said}");
    assert!(said.starts_with("tool\nseccomp=2 "), "{said}");
    // Beside it: refused by the kernel, as a shell says it, though it reads the file.
    let (status, said) = sh_under(&profile, &p.cwd, &format!("{} status", outside.display()));
    assert_eq!(status, Some(126), "{said}");
    assert!(said.is_empty(), "{said}");
    // The same, as the interpreter of a script it may start.
    let script = p.cwd.join("by-script");
    std::fs::write(&script, format!("#!{}\nstatus\n", outside.display())).expect("a script");
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("its mode");
    let (status, said) = sh_under(&profile, &p.cwd, &script.display().to_string());
    assert_eq!(status, Some(126), "{said}");
    // And through a link it may start from.
    let link = p.cwd.join("pointed");
    std::os::unix::fs::symlink(&outside, &link).expect("a link");
    let (status, said) = sh_under(&profile, &p.cwd, &format!("{} status", link.display()));
    assert_eq!(status, Some(126), "{said}");
}

#[test]
fn a_program_outside_where_the_profile_says_is_not_started_by_the_wrap() {
    let p = project();
    let outside = probe_copied_to(&p.outside);
    let out = wrap(purlis_linux_sandbox::line(
        &starts_only_from(&p.cwd, &p.cwd),
        outside.display().to_string(),
        vec!["status".to_owned()],
    ))
    .current_dir(&p.cwd)
    .output()
    .expect("the wrap ran");
    assert_eq!(out.status.code(), Some(127));
    assert!(out.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("could not be started"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_profile_that_names_no_start_starts_nothing() {
    let p = project();
    let out = wrap(purlis_linux_sandbox::line(
        "(version 1)\n(deny default)\n(allow file-read*)\n",
        "/bin/echo".to_owned(),
        vec!["started".to_owned()],
    ))
    .current_dir(&p.cwd)
    .output()
    .expect("the wrap ran");
    assert_eq!(out.status.code(), Some(127));
    assert!(out.stdout.is_empty());
}

#[test]
fn the_calls_round_the_supervisor_are_refused() {
    let mut p = project();
    let Ok(python) = which("python3") else {
        panic!("python3 is needed for this test's raw system calls");
    };
    let script = r#"
import ctypes, os, sys
libc = ctypes.CDLL(None, use_errno=True)
def call(nr, *args):
    r = libc.syscall(nr, *[ctypes.c_long(a) for a in args])
    return "ok" if r >= 0 else os.strerror(ctypes.get_errno())
import platform
x86 = platform.machine() == "x86_64"
SECCOMP = 317 if x86 else 277
KEYCTL = 250 if x86 else 219
ADD_KEY = 248 if x86 else 217
IO_URING_SETUP = 425
OPENAT2 = 437
print("listener", call(SECCOMP, 1, 8, 0))
print("keyctl", call(KEYCTL, 0, -3, 0))
print("add_key", call(ADD_KEY, 0, 0, 0, 0, 0))
print("io_uring", call(IO_URING_SETUP, 1, 0))
print("openat2", call(OPENAT2, -100, 0, 0, 0))
"#;
    let out = wrap(purlis_linux_sandbox::line(
        &{
            let port = p.proxy_port();
            p.profile(port)
        },
        python,
        vec!["-c".to_owned(), script.to_owned()],
    ))
    .current_dir(&p.cwd)
    .output()
    .expect("the wrap ran");
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(said.contains("listener Operation not permitted"), "{said}");
    assert!(said.contains("keyctl Operation not permitted"), "{said}");
    assert!(said.contains("add_key Operation not permitted"), "{said}");
    assert!(said.contains("io_uring Function not implemented"), "{said}");
    assert!(said.contains("openat2 Function not implemented"), "{said}");
}

#[test]
fn the_supervisor_s_own_process_is_out_of_reach() {
    let mut p = project();
    let said = p.run(&["read", "/proc/self/status"]);
    assert_eq!(said, "ok");
    // The wrap's own process: the probe's parent.
    let script = format!(
        "{PROBE} read /proc/$PPID/environ; {PROBE} read /proc/1/environ; {PROBE} read /proc/$$/status; \
         {PROBE} read /proc/$PPID/stat; {PROBE} read /proc/$PPID/cwd/x; {PROBE} read /proc/$PPID/task/$PPID/environ; \
         {PROBE} read /proc/$PPID/cmdline; {PROBE} read /proc/$PPID/status; {PROBE} read-at /proc/$PPID cmdline; \
         {PROBE} read-at /proc/$PPID stat; {PROBE} path-then-read /proc/$PPID/cmdline; \
         {PROBE} path-then-read /proc/$PPID/stat"
    );
    let out = wrap(purlis_linux_sandbox::line(
        &{
            let port = p.proxy_port();
            p.profile(port)
        },
        "/bin/sh".to_owned(),
        vec!["-c".to_owned(), script],
    ))
    .current_dir(&p.cwd)
    .output()
    .expect("the wrap ran");
    let said = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = said.lines().collect();
    // Of a process outside the sandbox, its `stat` alone, from the top of `/proc`, from its own
    // folder held already, or through a descriptor of one of its files held already.
    assert_eq!(
        lines,
        vec![
            "EACCES", "EACCES", "ok", "ok", "EACCES", "EACCES", "EACCES", "EACCES", "EACCES", "ok",
            "EACCES", "ok"
        ],
        "{said}"
    );
}

#[test]
fn a_file_held_by_name_stays_held_once_it_is_no_longer_linked() {
    let mut p = project();
    let key = p.cwd.join("held.key");
    std::fs::write(&key, "held").expect("a key");
    assert_eq!(p.run(&["read", "held.key"]), "EACCES");
    let port = p.proxy_port();
    let go = p.tmp.join("go");
    let reader = wrap(purlis_linux_sandbox::line(
        &p.profile(port),
        PROBE.to_owned(),
        vec!["path-wait-read".to_owned(), p.s(&key), p.s(&go)],
    ))
    .current_dir(&p.cwd)
    .stdout(std::process::Stdio::piped())
    .spawn()
    .expect("the wrap ran");
    std::thread::sleep(std::time::Duration::from_millis(500));
    std::fs::remove_file(&key).expect("unlinked");
    std::fs::write(&go, "").expect("go");
    let out = reader.wait_with_output().expect("it ends");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "EACCES");
}

#[test]
fn a_pipe_made_in_the_chat_s_folder_works() {
    let mut p = project();
    assert_eq!(p.run(&["mkfifo", "pipe"]), "ok");
    assert_eq!(p.run(&["mkfifo", &p.s(&p.outside.join("pipe"))]), "EACCES");
}

#[test]
fn the_wrap_ends_with_its_program_s_status() {
    let p = project();
    let out = wrap(purlis_linux_sandbox::line(
        &p.profile(1),
        PROBE.to_owned(),
        vec!["exit".to_owned(), "7".to_owned()],
    ))
    .current_dir(&p.cwd)
    .output()
    .expect("the wrap ran");
    assert_eq!(out.status.code(), Some(7));
}

#[test]
fn a_profile_the_wrap_cannot_read_starts_nothing() {
    let p = project();
    let out = wrap(purlis_linux_sandbox::line(
        "(version 1)\n(allow file-write* (path-regex \"x\"))\n",
        PROBE.to_owned(),
        vec!["write".to_owned(), "never".to_owned()],
    ))
    .current_dir(&p.cwd)
    .output()
    .expect("the wrap ran");
    assert_eq!(out.status.code(), Some(126));
    assert!(String::from_utf8_lossy(&out.stderr).contains("nothing was started"));
    assert!(!p.cwd.join("never").exists());
}

#[test]
fn the_program_is_given_exactly_the_environment_purlis_carried_for_it() {
    let p = project();
    let out = Command::new(PROBE)
        .args(purlis_linux_sandbox::line(
            &p.profile(1),
            "/usr/bin/env".to_owned(),
            Vec::new(),
        ))
        .current_dir(&p.cwd)
        .env_clear()
        .envs(purlis_linux_sandbox::carried::carried([
            ("PATH", "/usr/bin:/bin"),
            ("ONLY", "this=and that"),
        ]))
        .output()
        .expect("the wrap ran");
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{said}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut given: Vec<&str> = said.lines().collect();
    given.sort_unstable();
    assert_eq!(given, ["ONLY=this=and that", "PATH=/usr/bin:/bin"]);
}

#[test]
fn a_wrap_started_with_an_environment_purlis_did_not_build_starts_nothing() {
    let p = project();
    let words = purlis_linux_sandbox::line(
        &p.profile(1),
        PROBE.to_owned(),
        vec!["write".to_owned(), "never".to_owned()],
    );
    // The chat's own, inherited whole; and carried, with a variable of the chat's beside it.
    let inherited = Command::new(PROBE)
        .args(&words)
        .current_dir(&p.cwd)
        .output()
        .expect("the wrap ran");
    let beside = wrap(&words)
        .env("LD_LIBRARY_PATH", &p.tmp)
        .current_dir(&p.cwd)
        .output()
        .expect("the wrap ran");
    for out in [inherited, beside] {
        assert_eq!(out.status.code(), Some(126));
        let said = String::from_utf8_lossy(&out.stderr);
        assert!(
            said.contains("an environment purlis did not build")
                && said.contains("nothing was started"),
            "{said}"
        );
    }
    assert!(!p.cwd.join("never").exists());
}

fn which(program: &str) -> Result<String, ()> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|dir| dir.join(program))
        .find(|it| it.is_file())
        .map(|it| it.display().to_string())
        .ok_or(())
}
