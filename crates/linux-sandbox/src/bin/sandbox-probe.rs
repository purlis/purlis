// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A program for the Linux sandbox's tests, never shipped: as `sandbox-probe _sandbox …` it is
//! the wrap itself, exactly as `purlis _sandbox …` is; otherwise it tries one thing and prints
//! `ok` or the error's name, so a test reads what the sandbox let it do.

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    if let Some(code) = purlis_linux_sandbox::serve_if_asked() {
        std::process::exit(code);
    }
    let words: Vec<String> = std::env::args().skip(1).collect();
    let said = match probe::run(&words) {
        Ok(()) => "ok".to_owned(),
        Err(why) => why,
    };
    println!("{said}");
    std::process::ExitCode::SUCCESS
}

#[cfg(not(target_os = "linux"))]
fn main() {}

#[cfg(target_os = "linux")]
mod probe {
    use std::io::{Read, Write};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixStream;

    fn name(why: &std::io::Error) -> String {
        match why.raw_os_error() {
            Some(code) => format!("{:?}", nix::errno::Errno::from_raw(code)),
            None => why.to_string(),
        }
    }

    fn io<T>(got: std::io::Result<T>) -> Result<(), String> {
        got.map(drop).map_err(|why| name(&why))
    }

    fn nix<T>(got: nix::Result<T>) -> Result<(), String> {
        got.map(drop).map_err(|why| format!("{why:?}"))
    }

    pub(super) fn run(words: &[String]) -> Result<(), String> {
        let word = |at: usize| words.get(at).map(String::as_str).unwrap_or("");
        match word(0) {
            "write" => io(std::fs::write(word(1), b"x")),
            "append" => io(std::fs::OpenOptions::new()
                .append(true)
                .open(word(1))
                .and_then(|mut file| file.write_all(b"x"))),
            "read" => io(std::fs::read(word(1))),
            "list" => io(std::fs::read_dir(word(1)).map(|it| it.count())),
            "mkdir" => io(std::fs::create_dir(word(1))),
            "rename" => io(std::fs::rename(word(1), word(2))),
            "link" => io(std::fs::hard_link(word(1), word(2))),
            "symlink" => io(std::os::unix::fs::symlink(word(1), word(2))),
            "unlink" => io(std::fs::remove_file(word(1))),
            "rmdir" => io(std::fs::remove_dir(word(1))),
            "chmod" => io(std::fs::set_permissions(
                word(1),
                std::fs::Permissions::from_mode(u32::from_str_radix(word(2), 8).unwrap_or(0o755)),
            )),
            "truncate" => io(std::fs::OpenOptions::new()
                .write(true)
                .open(word(1))
                .and_then(|file| file.set_len(0))),
            "mkfifo" => nix(nix::unistd::mkfifo(
                word(1),
                nix::sys::stat::Mode::from_bits_truncate(0o644),
            )),
            "tmpfile-link" => {
                use std::os::unix::fs::OpenOptionsExt;
                let file = std::fs::OpenOptions::new()
                    .write(true)
                    .custom_flags(libc::O_TMPFILE)
                    .open(word(1))
                    .map_err(|why| name(&why))?;
                let at = format!("/proc/self/fd/{}", std::os::fd::AsRawFd::as_raw_fd(&file));
                nix(nix::unistd::linkat(
                    nix::fcntl::AT_FDCWD,
                    at.as_str(),
                    nix::fcntl::AT_FDCWD,
                    word(2),
                    nix::fcntl::AtFlags::AT_SYMLINK_FOLLOW,
                ))
            }
            "tcp" => io(std::net::TcpStream::connect((
                word(1),
                word(2).parse::<u16>().unwrap_or(0),
            ))
            .and_then(|mut stream| {
                stream.write_all(b"hi")?;
                let mut back = [0u8; 2];
                stream.read_exact(&mut back)
            })),
            "udp" => io(std::net::UdpSocket::bind("0.0.0.0:0").and_then(|socket| {
                socket.send_to(b"x", (word(1), word(2).parse::<u16>().unwrap_or(0)))
            })),
            "unix" => io(UnixStream::connect(word(1)).and_then(|mut stream| {
                stream.write_all(b"hi")?;
                let mut back = [0u8; 2];
                stream.read_exact(&mut back)
            })),
            // A socket that listens with no bind of its own: the kernel would bind it to a port
            // it picks, on every interface.
            "listen-unbound" => {
                let family = if word(1) == "6" {
                    nix::sys::socket::AddressFamily::Inet6
                } else {
                    nix::sys::socket::AddressFamily::Inet
                };
                nix(nix::sys::socket::socket(
                    family,
                    nix::sys::socket::SockType::Stream,
                    nix::sys::socket::SockFlag::empty(),
                    None,
                )
                .and_then(|socket| {
                    nix::sys::socket::listen(&socket, nix::sys::socket::Backlog::new(1)?)
                }))
            }
            // A thread with a descriptor table of its own listens on its own socket, never
            // bound, at the number where the process's table holds a bound one.
            "listen-unshared" => {
                use nix::sys::socket::{
                    AddressFamily, Backlog, SockFlag, SockType, UnixAddr, bind, listen, socket,
                };
                use std::os::fd::AsRawFd;
                let bound = socket(
                    AddressFamily::Unix,
                    SockType::Stream,
                    SockFlag::empty(),
                    None,
                )
                .map_err(|why| format!("{why:?}"))?;
                bind(
                    bound.as_raw_fd(),
                    &UnixAddr::new_abstract(word(1).as_bytes())
                        .map_err(|why| format!("{why:?}"))?,
                )
                .map_err(|why| format!("{why:?}"))?;
                std::thread::spawn(move || {
                    let mut at = bound;
                    nix(
                        nix::sched::unshare(nix::sched::CloneFlags::CLONE_FILES).and_then(|()| {
                            let own = socket(
                                AddressFamily::Inet,
                                SockType::Stream,
                                SockFlag::empty(),
                                None,
                            )?;
                            nix::unistd::dup2(&own, &mut at)?;
                            listen(&at, Backlog::new(1)?)
                        }),
                    )
                })
                .join()
                .unwrap_or_else(|_| Err("the thread panicked".to_owned()))
            }
            // A server started on an address and port, as a dev server is (#2019): bound and
            // listening, whatever it is bound to in the end.
            "serve-on" => io(std::net::TcpListener::bind((
                word(1),
                word(2).parse::<u16>().unwrap_or(0),
            ))),
            // A server on a unix socket at a path (#2019).
            "serve-at" => io(std::os::unix::net::UnixListener::bind(word(1))),
            // A server on an abstract unix socket: bound, then listening.
            "abstract-listen" => {
                use std::os::linux::net::SocketAddrExt;
                io(std::os::unix::net::SocketAddr::from_abstract_name(word(1))
                    .and_then(|at| std::os::unix::net::UnixListener::bind_addr(&at)))
            }
            "abstract" => {
                use std::os::linux::net::SocketAddrExt;
                let at = std::os::unix::net::SocketAddr::from_abstract_name(word(1))
                    .map_err(|why| name(&why))?;
                io(UnixStream::connect_addr(&at))
            }
            "signal-parent" => nix(nix::sys::signal::kill(
                nix::unistd::getppid(),
                nix::sys::signal::Signal::SIGCONT,
            )),
            "undumpable" => {
                nix(nix::sys::prctl::set_dumpable(false))?;
                // Still served: a write the profile allows is still made.
                io(std::fs::write(word(1), b"x"))
            }
            "status" => {
                let status =
                    std::fs::read_to_string("/proc/self/status").map_err(|why| name(&why))?;
                let field = |key: &str| {
                    status
                        .lines()
                        .find_map(|line| line.strip_prefix(key))
                        .map(|it| it.trim().to_owned())
                        .unwrap_or_default()
                };
                Err(format!(
                    "seccomp={} filters={} nnp={}",
                    field("Seccomp:"),
                    field("Seccomp_filters:"),
                    field("NoNewPrivs:")
                ))
            }
            "path-then-read" => {
                use std::os::unix::fs::OpenOptionsExt;
                let held = std::fs::OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_PATH)
                    .open(word(1))
                    .map_err(|why| name(&why))?;
                let again = format!("/proc/self/fd/{}", std::os::fd::AsRawFd::as_raw_fd(&held));
                io(std::fs::read(again))
            }
            "path-wait-read" => {
                use std::os::unix::fs::OpenOptionsExt;
                let held = std::fs::OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_PATH)
                    .open(word(1))
                    .map_err(|why| name(&why))?;
                while std::fs::symlink_metadata(word(2)).is_err() {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                let again = format!("/proc/self/fd/{}", std::os::fd::AsRawFd::as_raw_fd(&held));
                io(std::fs::read(again))
            }
            "read-at" => {
                use nix::fcntl::{OFlag, open, openat};
                use nix::sys::stat::Mode;
                let folder = open(word(1), OFlag::O_PATH | OFlag::O_DIRECTORY, Mode::empty())
                    .map_err(|why| format!("{why:?}"))?;
                nix(openat(&folder, word(2), OFlag::O_RDONLY, Mode::empty()))
            }
            "touch" => {
                use nix::sys::stat::{UtimensatFlags, utimensat};
                use nix::sys::time::TimeSpec;
                nix(utimensat(
                    nix::fcntl::AT_FDCWD,
                    word(1),
                    &TimeSpec::new(1, 0),
                    &TimeSpec::new(1, 0),
                    UtimensatFlags::FollowSymlink,
                ))
            }
            "chown" => nix(nix::unistd::chown(
                word(1),
                Some(nix::unistd::getuid()),
                Some(nix::unistd::getgid()),
            )),
            "exit" => std::process::exit(word(1).parse().unwrap_or(0)),
            other => Err(format!("no probe {other}")),
        }
    }
}
