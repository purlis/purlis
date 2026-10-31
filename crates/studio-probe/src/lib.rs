// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The studio probe**: an extension that adds a skill source, a lint, a template, a panel and
//! a skill router to the Skill studio (ADR 0101, ADR 0102), and one command, `try`, that attempts one thing from inside
//! its sandbox and prints what happened, so a test reads what the confinement let it do.
//!
//! Test-only, and written as a stranger would write an extension: it knows the protocol and
//! nothing of purlis's insides. `studio-probe assemble <dir>` puts the program and its manifest
//! in `<dir>`, as an author would lay an extension out.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// The protocol it speaks.
pub const PROTOCOL: u64 = 3;

/// Its manifest.
pub const MANIFEST: &str = include_str!("../purlis-extension.json");

/// The skill its source offers.
pub const OFFERED: &str = "probe-skill";

/// The revision its source names what it holds by.
pub const REVISION: &str = "probe-r1";

/// Lay the probe out in `dir`: its manifest, and this program as `bin/studio-probe`.
pub fn assemble(dir: &Path) -> std::io::Result<PathBuf> {
    let me = std::env::current_exe()?;
    std::fs::create_dir_all(dir.join("bin"))?;
    std::fs::create_dir_all(dir.join("state"))?;
    std::fs::write(dir.join("purlis-extension.json"), MANIFEST)?;
    let program = dir.join("bin").join("studio-probe");
    // A rename over the old copy, never a write through it: on macOS writing a program that has
    // run invalidates its signature.
    let beside = dir.join("bin").join(".studio-probe.new");
    std::fs::copy(&me, &beside)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&beside, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&beside, &program)?;
    Ok(program)
}

/// The answer to one request line, and, for a command, its exit status: `None` for an answer
/// line, which is printed with a newline.
pub fn answer(request: &serde_json::Value) -> (String, Option<u8>) {
    if request.get("command").and_then(serde_json::Value::as_str) == Some("try") {
        let words: Vec<String> = request
            .get("args")
            .and_then(serde_json::Value::as_array)
            .map(|args| {
                args.iter()
                    .filter_map(|it| it.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        return (format!("{}\n", tried(&words)), Some(0));
    }
    let said = match request.get("studio").and_then(serde_json::Value::as_str) {
        Some(_) => studio(request.get("ask").unwrap_or(&serde_json::Value::Null)),
        None => serde_json::json!({ "error": "the probe answers studio questions and `try`" }),
    };
    let mut said = said;
    said["charter"] = PROTOCOL.into();
    (said.to_string(), None)
}

/// A studio question's answer.
fn studio(ask: &serde_json::Value) -> serde_json::Value {
    let word = |key: &str| {
        ask.get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
    };
    let files = ask
        .get("files")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    match word("ask") {
        "skills" => serde_json::json!({
            "revision": REVISION,
            "skills": [{ "name": OFFERED, "description": "A skill the studio probe offers, for its tests." }],
        }),
        "files" => serde_json::json!({
            "revision": word("revision"),
            "files": [
                {
                    "path": "SKILL.md",
                    "text": format!(
                        "---\nname: {OFFERED}\ndescription: A skill the studio probe offers, for its tests.\n---\n\n# {OFFERED}\n\nRun `scripts/check.sh` when asked.\n"
                    ),
                },
                { "path": "scripts/check.sh", "text": "#!/bin/sh\necho checked\n", "executable": true },
            ],
        }),
        "findings" => serde_json::json!({
            "findings": [{ "says": format!("the probe was handed {files} files of {}", word("skill")) }],
        }),
        "findings-of-each" => findings_of_each(ask),
        "template" => serde_json::json!({
            "text": format!(
                "---\nname: {}\ndescription: {}\n---\n\n# {}\n\nWritten by the studio probe.\n",
                word("name"),
                word("description"),
                word("name")
            ),
        }),
        "panel" => serde_json::json!({
            "blocks": [
                { "kind": "note", "text": format!("The probe's panel on {}.", word("skill")) },
                {
                    "kind": "table",
                    "caption": "Probe",
                    "columns": ["Handed", "Files"],
                    "rows": [{ "key": "handed", "cells": ["files", files.to_string()] }],
                },
            ],
        }),
        "route" => routed(ask),
        other => serde_json::json!({ "error": format!("the probe has no answer to {other:?}") }),
    }
}

/// **A lint's answer about many skills at once** (#4195): for each skill asked, in order, how
/// many files it was handed and how many skills the one question held.
fn findings_of_each(ask: &serde_json::Value) -> serde_json::Value {
    let skills = ask
        .get("skills")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let many = skills.len();
    let answered: Vec<serde_json::Value> = skills
        .iter()
        .map(|one| {
            let name = one
                .get("skill")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let files = one
                .get("files")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len);
            serde_json::json!({
                "skill": name,
                "findings": [{ "says": format!("the probe was handed {files} files of {name} in one question of {many}") }],
            })
        })
        .collect();
    serde_json::json!({ "skills": answered })
}

/// The skill whose name makes the probe's router take longer than any router has.
pub const SLOW: &str = "probe-slow";

/// The start of the names the probe's router hides.
pub const HIDES: &str = "probe-hide";

/// **A router's answer**: every skill it was asked about whose name starts with [`HIDES`], each
/// "hidden by the probe"; after two seconds where one is named [`SLOW`].
fn routed(ask: &serde_json::Value) -> serde_json::Value {
    let names: Vec<&str> = ask
        .get("skills")
        .and_then(serde_json::Value::as_array)
        .map(|skills| {
            skills
                .iter()
                .filter_map(|one| one.get("name").and_then(serde_json::Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    if names.contains(&SLOW) {
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    let hidden: Vec<serde_json::Value> = names
        .iter()
        .filter(|name| name.starts_with(HIDES))
        .map(|name| serde_json::json!({ "skill": name, "why": "hidden by the probe" }))
        .collect();
    serde_json::json!({ "hidden": hidden })
}

/// **One thing tried from inside the sandbox**, and what happened, as one line: `ok` and what it
/// saw, or what refused it.
///
/// - `read <path>`: read a file.
/// - `write <path>`: write a file.
/// - `connect <address:port>`: connect to an address directly, past any proxy.
/// - `link <from> <to>` and `symlink <from> <to>`: make a link at `<to>` to `<from>`.
/// - `hello <host> <name>`: ask its proxy for a tunnel to `<host>:443`, send a TLS ClientHello
///   naming `<name>` through it, and say whether anything came back.
/// - `environ <pid>`: what `ps -wwE` shows of another process's command and environment.
/// - `proxy <host>`: ask its proxy (`HTTPS_PROXY`) for a tunnel to `<host>:443`, and say the
///   proxy's status line.
/// - `start <program> <args>…`: start a program, and say how it ended and what it printed.
/// - `start-linked <program> <at>`: the same through a symbolic link made at `<at>`, which it
///   takes away again.
/// - `start-held <program>` on Linux: the same through a path-only descriptor of it.
pub fn tried(words: &[String]) -> String {
    let said = match words {
        [verb, path] if verb == "read" => std::fs::read(path)
            .map(|bytes| format!("ok, {} bytes", bytes.len()))
            .map_err(|why| why.kind().to_string()),
        [verb, path] if verb == "write" => std::fs::write(path, "probe\n")
            .map(|()| "ok".to_owned())
            .map_err(|why| why.kind().to_string()),
        [verb, address] if verb == "connect" => {
            match address
                .parse::<std::net::SocketAddr>()
                .map_err(|why| why.to_string())
            {
                Ok(address) => std::net::TcpStream::connect_timeout(
                    &address,
                    std::time::Duration::from_secs(3),
                )
                .map(|_| "ok, connected".to_owned())
                .map_err(|why| why.kind().to_string()),
                Err(why) => Err(why),
            }
        }
        [verb, from, to] if verb == "link" => std::fs::hard_link(from, to)
            .map(|()| "ok".to_owned())
            .map_err(|why| why.kind().to_string()),
        #[cfg(unix)]
        [verb, from, to] if verb == "symlink" => std::os::unix::fs::symlink(from, to)
            .map(|()| "ok".to_owned())
            .map_err(|why| why.kind().to_string()),
        [verb, host] if verb == "proxy" => through_the_proxy(host),
        [verb, host, named] if verb == "hello" => hello_through_the_proxy(host, named),
        [verb, pid] if verb == "environ" => environ(pid),
        [verb, program, args @ ..] if verb == "start" => started(program, args),
        #[cfg(unix)]
        [verb, program, at] if verb == "start-linked" => std::os::unix::fs::symlink(program, at)
            .map_err(|why| why.kind().to_string())
            .and_then(|()| {
                let said = started(at, &[]);
                let _ = std::fs::remove_file(at);
                said
            }),
        #[cfg(target_os = "linux")]
        [verb, program] if verb == "start-held" => {
            use std::os::unix::fs::OpenOptionsExt as _;
            use std::os::unix::io::AsRawFd as _;
            /// `O_PATH`, the same number on every architecture purlis runs on.
            const PATH_ONLY: i32 = 0o10000000;
            std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(PATH_ONLY)
                .open(program)
                .map_err(|why| why.kind().to_string())
                .and_then(|held| started(&format!("/proc/self/fd/{}", held.as_raw_fd()), &[]))
        }
        _ => Err(format!("nothing to try in {words:?}")),
    };
    match said {
        Ok(it) => it,
        Err(why) => format!("refused: {why}"),
    }
}

/// How `program` started with `args` ended and what it printed, or why it did not start.
fn started(program: &str, args: &[String]) -> Result<String, String> {
    std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .map(|out| {
            format!(
                "ok, it ended {} and said {}",
                out.status,
                String::from_utf8_lossy(&out.stdout).trim()
            )
        })
        .map_err(|why| why.kind().to_string())
}

/// The proxy's status line for a tunnel to `host`, or why there was none.
fn through_the_proxy(host: &str) -> Result<String, String> {
    let proxy = std::env::var("HTTPS_PROXY").map_err(|_| "no proxy".to_owned())?;
    let at = proxy
        .strip_prefix("http://")
        .ok_or_else(|| format!("a proxy that is not http: {proxy}"))?
        .trim_end_matches('/');
    let mut stream = std::net::TcpStream::connect(at).map_err(|why| why.kind().to_string())?;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(20)));
    write!(
        stream,
        "CONNECT {host}:443 HTTP/1.1\r\nHost: {host}:443\r\n\r\n"
    )
    .map_err(|why| why.kind().to_string())?;
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while head.len() < 512 && !head.ends_with(b"\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => break,
        }
    }
    Ok(format!(
        "proxy said {}",
        String::from_utf8_lossy(&head).trim()
    ))
}

/// The proxy's status line for a tunnel to `host`, then whether anything came back once a TLS
/// ClientHello naming `named` was sent through it.
fn hello_through_the_proxy(host: &str, named: &str) -> Result<String, String> {
    let proxy = std::env::var("HTTPS_PROXY").map_err(|_| "no proxy".to_owned())?;
    let at = proxy
        .strip_prefix("http://")
        .ok_or_else(|| format!("a proxy that is not http: {proxy}"))?
        .trim_end_matches('/');
    let mut stream = std::net::TcpStream::connect(at).map_err(|why| why.kind().to_string())?;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(20)));
    write!(
        stream,
        "CONNECT {host}:443 HTTP/1.1\r\nHost: {host}:443\r\n\r\n"
    )
    .map_err(|why| why.kind().to_string())?;
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while head.len() < 4096 && !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => break,
        }
    }
    let status = String::from_utf8_lossy(&head)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();
    if !status.contains(" 200 ") {
        return Ok(format!("proxy said {status}"));
    }
    stream
        .write_all(&client_hello(named))
        .map_err(|why| why.kind().to_string())?;
    let mut back = [0u8; 512];
    let came = stream.read(&mut back).unwrap_or(0);
    Ok(format!("proxy said {status}; {came} bytes came back"))
}

/// A TLS record holding a ClientHello naming `name`, as a client writes one.
fn client_hello(name: &str) -> Vec<u8> {
    let name = name.as_bytes();
    let len = |n: usize| u16::try_from(n).unwrap_or(u16::MAX).to_be_bytes();
    let mut list = Vec::new();
    list.extend(len(3 + name.len()));
    list.push(0);
    list.extend(len(name.len()));
    list.extend(name);
    let mut extensions = Vec::new();
    extensions.extend(43u16.to_be_bytes());
    extensions.extend(5u16.to_be_bytes());
    extensions.extend([4, 3, 4, 3, 3]);
    extensions.extend(0u16.to_be_bytes());
    extensions.extend(len(list.len()));
    extensions.extend(list);
    let mut hello = vec![3, 3];
    hello.extend([7u8; 32]);
    hello.push(0);
    hello.extend(4u16.to_be_bytes());
    hello.extend([0x13, 0x01, 0xc0, 0x2f]);
    hello.extend([1, 0]);
    hello.extend(len(extensions.len()));
    hello.extend(extensions);
    let mut handshake = vec![1, 0];
    handshake.extend(len(hello.len()));
    handshake.extend(hello);
    let mut record = vec![0x16, 3, 1];
    record.extend(len(handshake.len()));
    record.extend(handshake);
    record
}

/// Another process's command and environment, as this system shows them: `ps -wwE` on macOS,
/// which reads them through the system's settings, and its `/proc` files on Linux.
fn environ(pid: &str) -> Result<String, String> {
    if cfg!(target_os = "macos") {
        return std::process::Command::new("ps")
            .args(["-wwE", "-o", "command=", "-p", pid])
            .output()
            .map(|out| format!("ok, {}", String::from_utf8_lossy(&out.stdout).trim()))
            .map_err(|why| why.kind().to_string());
    }
    let read = |name: &str| {
        std::fs::read(format!("/proc/{pid}/{name}"))
            .map(|bytes| String::from_utf8_lossy(&bytes).replace('\0', " "))
            .map_err(|why| why.kind().to_string())
    };
    Ok(format!("ok, {} {}", read("cmdline")?, read("environ")?))
}
