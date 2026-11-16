// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **R-772's CI half: what a chat's command, hook and MCP server reach of the keychain**
//! (T1a-72, #6764; decisions.md D6640-T3 X7 and item 7).
//!
//! For the system it runs on, at each sandbox level and for each harness, a project with a model
//! key in your keys and no keyring vault has its chat's sandbox decided as a start decides it
//! ([`sandbox::decide`]), and its profile built by the harness's own compiler, never written by
//! hand. Then the checked-in probe (`tests/keychain_reach/probe.sh`, `probe.ps1` on Windows) runs
//! as each caller under that profile and says whether it reaches the keychain service and the test
//! build's stub keyring. Every cell is compared with the pinned table
//! (`tests/keychain_reach/pinned-table.md`), and any cell that differs fails, naming it.
//!
//! - **A command** is the probe run by the chat's line; **a hook** is the probe run through a shell
//!   line, as a harness runs a hook's command; **an MCP server** is the probe started with a
//!   client's request on its input, as a stdio server is. A harness purlis wraps starts all three
//!   inside its wrap. Claude Code's own sandbox on macOS holds only the commands it runs: its hooks
//!   and MCP servers run outside any profile, and are run so here, and its commands' cells need the
//!   real Claude Code, so they are the hand run's.
//! - **macOS**: the keychain service's cell is the profile's rule for the security server's Mach
//!   service ([`security_server`]); the stub's are measured under `sandbox-exec`. It runs in
//!   `studio-confinement-macos`. A process that cannot apply a profile (already sandboxed) checks
//!   only the profile's cells, and on CI that is a failure.
//! - **Linux**: every cell is measured under purlis's own wrap (`sandbox-probe`, as the runner's
//!   link-key test runs it). The service cell is whether `org.freedesktop.secrets` answers on the
//!   session bus, so the run needs a session bus with an unlocked Secret Service on it, and says
//!   so: `PURLIS_TEST_SECRET_SERVICE=1` asks for the live run (ci.yml's `rust` job starts one with
//!   `dbus-run-session` and `gnome-keyring-daemon --unlock`), and outside the sandbox the probe must
//!   reach it, or a refusal inside would say nothing. Without it only the profiles are built.
//! - **Windows**: purlis has no sandbox there; the cells are measured and a differing one is
//!   reported, never failed (the nightly `windows` job is evidence, X7).
//!
//! The item cells (denied, prompted, readable) are the hand run's on a signed build: a test build
//! keeps its keychain in the stub file and can't reach the system's (`secrets::keyring`).

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use purlis_core::harness::Harness;
use purlis_core::sandbox::{self, Decided, Form, Machine, Os};
use purlis_core::seam::{Record, RecordingCredentials};
use purlis_core::secrets::keyring::{FileStore, STUB_FILE, Store};
use purlis_core::yourkeys::{self, Checked, Product};

/// The pinned table, the one file T1a-17 posts.
const PINNED: &str = include_str!("../keychain_reach/pinned-table.md");

/// The folder of the probe the table is measured with.
const PROBES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/keychain_reach");

/// Each level's name in the table, and the manifest of a project that starts its chats there.
const LEVELS: [(&str, &str); 2] = [
    (
        "full",
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n",
    ),
    ("floor", "schema = 1\n"),
];

/// Each harness's name in the table.
const HARNESSES: [(&str, Harness); 3] = [
    ("opencode", Harness::Opencode),
    ("codex", Harness::Codex),
    ("claude-code", Harness::ClaudeCode),
];

/// The callers inside a chat.
const CALLERS: [&str; 3] = ["command", "hook", "mcp"];

/// What an MCP client sends a stdio server first.
const MCP_REQUEST: &str =
    "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n";

/// A cell no test build can measure.
const HAND_RUN: &str = "hand-run";

/// One row of the table.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Row {
    os: String,
    level: String,
    harness: String,
    caller: String,
    service: String,
    stub_read: String,
    stub_write: String,
}

impl std::fmt::Display for Row {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "| {} | {} | {} | {} | {} | {} | {} |",
            self.os,
            self.level,
            self.harness,
            self.caller,
            self.service,
            self.stub_read,
            self.stub_write
        )
    }
}

/// The CI half's rows the pinned table holds for `os`.
fn pinned(os: &str) -> Vec<Row> {
    let start = PINNED
        .find("<!-- ci-table:start -->")
        .expect("the CI table's start");
    let end = PINNED
        .find("<!-- ci-table:end -->")
        .expect("the CI table's end");
    PINNED[start..end]
        .lines()
        .filter(|line| {
            line.starts_with("| ") && !line.starts_with("| OS ") && !line.starts_with("| ---")
        })
        .map(|line| {
            let cells: Vec<String> = line
                .trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().to_owned())
                .collect();
            assert_eq!(cells.len(), 7, "a row of seven cells: {line}");
            Row {
                os: cells[0].clone(),
                level: cells[1].clone(),
                harness: cells[2].clone(),
                caller: cells[3].clone(),
                service: cells[4].clone(),
                stub_read: cells[5].clone(),
                stub_write: cells[6].clone(),
            }
        })
        .filter(|row| row.os == os)
        .collect()
}

/// This system's name in the table.
fn this_os() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "windows"
    }
}

/// **The profile's rule for the security server's Mach service**: `reachable` where the last rule
/// that speaks of a lookup of `com.apple.SecurityServer` allows it, `denied` where it denies it.
/// Seatbelt takes the last rule that matches, and a profile starts from `(deny default)` or
/// `(allow default)`. A rule names the service by its whole name, by a prefix of it, or not at all
/// (every service).
fn security_server(profile: &str) -> &'static str {
    const SERVICE: &str = "com.apple.SecurityServer";
    let mut said = if profile.contains("(deny default)") {
        "denied"
    } else if profile.contains("(allow default)") {
        "reachable"
    } else {
        panic!("a profile that starts from neither default:\n{profile}");
    };
    // Each rule in order, by where it starts.
    let mut rules: Vec<(usize, &'static str, &str)> = Vec::new();
    for (verb, outcome) in [
        ("(allow mach-lookup", "reachable"),
        ("(deny mach-lookup", "denied"),
    ] {
        let mut from = 0;
        while let Some(at) = profile[from..].find(verb) {
            let begin = from + at;
            let body = rule_at(profile, begin);
            rules.push((begin, outcome, body));
            from = begin + verb.len();
        }
    }
    rules.sort_by_key(|(begin, _, _)| *begin);
    for (_, outcome, body) in rules {
        let filters = body.trim_start_matches('(');
        let names_it = !filters.contains("global-name")
            || filters.contains(&format!("(global-name \"{SERVICE}\")"))
            || prefixes(filters)
                .iter()
                .any(|prefix| SERVICE.starts_with(prefix));
        if names_it {
            said = outcome;
        }
    }
    said
}

/// The whole rule that opens at `begin`, to its closing parenthesis.
fn rule_at(profile: &str, begin: usize) -> &str {
    let mut depth = 0usize;
    let mut quoted = false;
    for (at, c) in profile[begin..].char_indices() {
        match c {
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => {
                depth -= 1;
                if depth == 0 {
                    return &profile[begin..=begin + at];
                }
            }
            _ => {}
        }
    }
    panic!("a rule with no end at {begin}");
}

/// The prefixes a rule's `global-name-prefix` filters name.
fn prefixes(rule: &str) -> Vec<&str> {
    rule.match_indices("(global-name-prefix \"")
        .filter_map(|(at, open)| {
            let rest = &rule[at + open.len()..];
            rest.find('"').map(|end| &rest[..end])
        })
        .collect()
}

/// How a caller is held.
enum Held {
    /// Outside any profile.
    Nothing,
    /// Under a Seatbelt profile, by `sandbox-exec`.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Seatbelt(String),
    /// Under purlis's own Linux wrap: the wrap's program and the profile.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Wrapped(PathBuf, String),
}

/// What the probe said as `caller`, asked `words`, held by `held`, in `cwd` with `env`.
fn probed(
    held: &Held,
    caller: &str,
    words: &[String],
    cwd: &Path,
    env: &[(String, String)],
) -> BTreeMap<String, String> {
    let (program, args) = caller_line(caller, words);
    let mut command = match held {
        Held::Nothing => {
            let mut command = Command::new(&program);
            command.args(&args).env_clear().envs(env.iter().cloned());
            command
        }
        Held::Seatbelt(profile) => {
            let mut command = Command::new("/usr/bin/sandbox-exec");
            command
                .args(["-p", profile, &program])
                .args(&args)
                .env_clear()
                .envs(env.iter().cloned());
            command
        }
        #[cfg(target_os = "linux")]
        Held::Wrapped(wrap, profile) => {
            let mut command = Command::new(wrap);
            command
                .args(purlis_linux_sandbox::line(profile, program, args))
                .env_clear()
                .envs(purlis_linux_sandbox::carried::carried(env.iter().cloned()));
            command
        }
        #[cfg(not(target_os = "linux"))]
        Held::Wrapped(..) => unreachable!("purlis's own wrap is Linux's"),
    };
    command
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = purlis_core::forklock::spawn(&mut command).expect("the probe starts");
    {
        let mut input = child.stdin.take().expect("its input");
        if caller == "mcp" {
            let _ = input.write_all(MCP_REQUEST.as_bytes());
        }
    }
    let out = child.wait_with_output().expect("the probe ends");
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    let answers: BTreeMap<String, String> = said
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect();
    assert_eq!(
        answers.get("caller").map(String::as_str),
        Some(caller),
        "the probe ran as {caller} and said so: {said}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    answers
}

/// The program and words each caller is started with.
#[cfg(unix)]
fn caller_line(caller: &str, words: &[String]) -> (String, Vec<String>) {
    let probe = format!("{PROBES}/probe.sh");
    let mut args = match caller {
        // A hook's command, run through a shell line as a harness runs it.
        "hook" => vec![
            "-c".to_owned(),
            "exec /bin/sh \"$0\" --as hook \"$@\"".to_owned(),
            probe,
        ],
        other => vec![probe, "--as".to_owned(), other.to_owned()],
    };
    args.extend(words.iter().cloned());
    ("/bin/sh".to_owned(), args)
}

/// The program and words each caller is started with.
#[cfg(windows)]
fn caller_line(caller: &str, words: &[String]) -> (String, Vec<String>) {
    let mut args = vec![
        "-NoProfile".to_owned(),
        "-ExecutionPolicy".to_owned(),
        "Bypass".to_owned(),
        "-File".to_owned(),
        format!("{PROBES}/probe.ps1"),
        "-As".to_owned(),
        caller.to_owned(),
    ];
    args.extend(words.iter().cloned());
    ("powershell".to_owned(), args)
}

/// One Anthropic key added to your keys under `config`, through the window's write, against the
/// fenced stub keyring purlis's config folder holds.
fn a_model_key(config: &Path) -> PathBuf {
    let stub = purlis_core::machine::dir(config).join(STUB_FILE);
    let credentials =
        RecordingCredentials::new(Box::new(FileStore::at(stub.clone())), Record::default());
    yourkeys::add(
        config,
        &credentials,
        Product::anthropic(),
        Checked::Passed,
        "synthetic-model-key-for-the-reach-table",
    )
    .expect("the key is added");
    assert!(stub.is_file(), "the stub keyring is written");
    stub
}

/// A chat's place: its project, with a stub keyring in its state folder, and a temporary folder.
fn a_project(
    out: &Path,
    level: &str,
    harness: &str,
    manifest: &str,
) -> (PathBuf, PathBuf, PathBuf) {
    let root = out.join(level).join(harness).join("project");
    let tmp = out.join(level).join(harness).join("tmp");
    std::fs::create_dir_all(&root).expect("a project");
    std::fs::create_dir_all(&tmp).expect("a temporary folder");
    std::fs::write(root.join("purlis.toml"), manifest).expect("the manifest");
    let stub = root.join(".purlis").join(STUB_FILE);
    std::fs::create_dir_all(stub.parent().expect("the state folder")).expect("the state folder");
    FileStore::at(stub.clone())
        .set("purlis/ops/3f9a2c1b", "API_TOKEN", "synthetic-stub-value")
        .expect("the stub keyring is written");
    (root, tmp, stub)
}

/// The profile `applied` gives a chat in `root` with the temporary folder `tmp`, or `None` where
/// the harness's own sandbox holds its commands and nothing holds its hooks and MCP servers.
fn the_profile(applied: &sandbox::Applied, root: &Path, tmp: &Path) -> Option<String> {
    let profile = match applied.form_in(Some(root)) {
        Form::Opencode(wrap) => sandbox::opencode::profile(&wrap, root, tmp, &[], None),
        Form::Codex(wrap) => sandbox::codex::profile(&wrap, root, tmp, &[], None),
        Form::ClaudeWrapped(wrap) => sandbox::claude_wrap::profile(&wrap, root, tmp, &[], None),
        Form::ClaudeCode(_) => return None,
    };
    Some(profile.expect("a profile"))
}

/// Whether this process can hold a child at all: not where it is itself sandboxed (macOS), and
/// where the wrap and the Secret Service the run needs are there (Linux).
fn the_hold() -> Option<Held> {
    #[cfg(target_os = "macos")]
    {
        let applies = purlis_core::forklock::status(Command::new("/usr/bin/sandbox-exec").args([
            "-p",
            "(version 1)(allow default)",
            "/usr/bin/true",
        ]))
        .is_ok_and(|status| status.success());
        if !applies {
            assert!(
                std::env::var_os("CI").is_none(),
                "this process cannot apply a Seatbelt profile on CI"
            );
            return None;
        }
        Some(Held::Seatbelt(String::new()))
    }
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("PURLIS_TEST_SECRET_SERVICE").is_none() {
            return None;
        }
        assert!(
            std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some(),
            "PURLIS_TEST_SECRET_SERVICE asks for the live run: start a session bus first \
             (dbus-run-session) with an unlocked Secret Service on it (gnome-keyring-daemon --unlock)"
        );
        assert!(
            purlis_linux_sandbox::abi() >= purlis_linux_sandbox::MIN_ABI,
            "this kernel's Landlock is version {}, and purlis's Linux sandbox needs {}",
            purlis_linux_sandbox::abi(),
            purlis_linux_sandbox::MIN_ABI
        );
        let wrap = match std::env::var_os("PURLIS_SANDBOX_PROBE") {
            Some(named) => PathBuf::from(named),
            None => purlis_core::testrun::bin_dir()
                .expect("this build's folder")
                .join("sandbox-probe"),
        };
        assert!(wrap.is_file(), "no sandbox-probe at {}", wrap.display());
        Some(Held::Wrapped(wrap, String::new()))
    }
    #[cfg(windows)]
    {
        Some(Held::Nothing)
    }
}

/// The environment every caller starts with.
fn env_of(tmp: &Path) -> Vec<(String, String)> {
    let mut env = vec![
        ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ("TMPDIR".to_owned(), tmp.display().to_string()),
    ];
    if cfg!(windows) {
        env = std::env::vars()
            .filter(|(key, _)| {
                ["PATH", "SYSTEMROOT", "WINDIR", "TEMP", "TMP", "USERPROFILE"]
                    .iter()
                    .any(|kept| key.eq_ignore_ascii_case(kept))
            })
            .collect();
    }
    if let Ok(bus) = std::env::var(purlis_core::chatenv::SESSION_BUS) {
        env.push((purlis_core::chatenv::SESSION_BUS.to_owned(), bus));
    }
    env
}

#[test]
fn the_keychain_service_reach_table_matches_the_pinned_table() {
    purlis_core::unsteered!();
    let os = this_os();
    let scratch = tempfile::tempdir().expect("a folder");
    let out = scratch.path().canonicalize().expect("a real folder");
    let home = out.join("home");
    let config = home.join(".config");
    std::fs::create_dir_all(&config).expect("a config home");
    let config_stub = a_model_key(&config);
    let machine = Machine {
        env: purlis_core::secrets::Env::of(&[("PATH", "/usr/bin:/bin")]),
        home: Some(home.clone()),
        os: Os::this(),
    };
    let hold = the_hold();
    let live = hold.is_some();

    // Outside any profile the probe must reach what it probes, or a refusal inside says nothing.
    if live {
        let tmp = out.join("outside");
        std::fs::create_dir_all(&tmp).expect("a folder");
        let stub = config_stub.display().to_string();
        let words = |w: &[&str]| w.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>();
        let reach = probed(
            &Held::Nothing,
            "command",
            &words(&["reach"]),
            &tmp,
            &env_of(&tmp),
        );
        if os != "macos" {
            assert_eq!(
                reach.get("service").map(String::as_str),
                Some("reachable"),
                "outside any sandbox the keychain service must answer: {reach:?}"
            );
        }
        let stubs = probed(
            &Held::Nothing,
            "command",
            &words(&["stub", &stub]),
            &tmp,
            &env_of(&tmp),
        );
        assert_eq!(
            (
                stubs.get("stub-read").map(String::as_str),
                stubs.get("stub-write").map(String::as_str)
            ),
            (Some("reachable"), Some("reachable")),
            "outside any sandbox the stub keyring is read and written: {stubs:?}"
        );
    }

    let mut measured: Vec<Row> = Vec::new();
    let windows = os == "windows";
    let levels: Vec<(&str, &str)> = if windows {
        vec![("none", "")]
    } else {
        LEVELS.to_vec()
    };
    let harnesses: Vec<(&str, Option<Harness>)> = if windows {
        vec![("any", None)]
    } else {
        HARNESSES
            .iter()
            .map(|(name, harness)| (*name, Some(*harness)))
            .collect()
    };
    for (level, manifest) in &levels {
        for (name, harness) in &harnesses {
            let (root, tmp, plane_stub) = a_project(&out, level, name, manifest);
            let profile = harness.map(|harness| {
                let decided = sandbox::decide(harness, &root, &machine, &|_| true, None, None)
                    .expect("decided")
                    .expect("a sandbox in force");
                let Decided::Sandboxed(applied) = decided else {
                    panic!("{level} {name}: not sandboxed: {decided:?}");
                };
                assert_eq!(
                    applied.level().title(),
                    if *level == "full" {
                        "full sandbox"
                    } else {
                        "basic sandbox"
                    },
                    "{level} {name}"
                );
                the_profile(&applied, &root, &tmp)
            });
            let profile = profile.flatten();
            let stubs = vec![
                "stub".to_owned(),
                plane_stub.display().to_string(),
                config_stub.display().to_string(),
            ];
            for caller in CALLERS {
                let mut row = Row {
                    os: os.to_owned(),
                    level: (*level).to_owned(),
                    harness: (*name).to_owned(),
                    caller: caller.to_owned(),
                    service: HAND_RUN.to_owned(),
                    stub_read: HAND_RUN.to_owned(),
                    stub_write: HAND_RUN.to_owned(),
                };
                // Claude Code's own sandbox holds its commands, which need the real Claude Code.
                let own_sandbox = !windows && profile.is_none();
                if own_sandbox && caller == "command" {
                    measured.push(row);
                    continue;
                }
                let held = match (&hold, &profile) {
                    (None, _) => None,
                    (Some(_), None) => Some(Held::Nothing),
                    (Some(Held::Seatbelt(_)), Some(profile)) => {
                        Some(Held::Seatbelt(profile.clone()))
                    }
                    (Some(Held::Wrapped(wrap, _)), Some(profile)) => {
                        Some(Held::Wrapped(wrap.clone(), profile.clone()))
                    }
                    (Some(Held::Nothing), Some(_)) => unreachable!("Windows has no profile"),
                };
                if os == "macos" {
                    row.service = profile
                        .as_deref()
                        .map_or("reachable", security_server)
                        .to_owned();
                }
                if let Some(held) = held {
                    let env = env_of(&tmp);
                    if os != "macos" {
                        let reach = probed(&held, caller, &["reach".to_owned()], &root, &env);
                        row.service = reach.get("service").cloned().unwrap_or_default();
                    }
                    let said = probed(&held, caller, &stubs, &root, &env);
                    row.stub_read = said.get("stub-read").cloned().unwrap_or_default();
                    row.stub_write = said.get("stub-write").cloned().unwrap_or_default();
                }
                measured.push(row);
            }
        }
    }

    let pinned = pinned(os);
    assert_eq!(
        pinned.len(),
        measured.len(),
        "the pinned table has a row for every cell this system measures"
    );
    let mut differing: Vec<String> = Vec::new();
    for (pinned, measured) in pinned.iter().zip(&measured) {
        let same_place = (&pinned.level, &pinned.harness, &pinned.caller)
            == (&measured.level, &measured.harness, &measured.caller);
        assert!(
            same_place,
            "the pinned table's rows are in the order measured: {pinned} against {measured}"
        );
        // Without a live run, only the cells the profile alone answers are compared.
        let compared = if live {
            pinned == measured
        } else {
            measured.service == HAND_RUN || pinned.service == measured.service
        };
        if !compared {
            differing.push(format!("pinned   {pinned}\nmeasured {measured}"));
        }
    }
    if windows {
        // Evidence, never a gate (X7): each differing cell is said, and the test passes.
        for one in &differing {
            println!("::warning title=R-772 table::{one}");
        }
        return;
    }
    assert!(
        differing.is_empty(),
        "cells differ from tests/keychain_reach/pinned-table.md:\n{}",
        differing.join("\n")
    );
    if live {
        let ran = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tests-that-really-ran");
        let _ = std::fs::create_dir_all(&ran);
        let _ = std::fs::write(ran.join("keychain-reach-table"), "");
    } else {
        let _ = writeln!(
            std::io::stderr(),
            "only the cells the profiles answer were compared: no sandbox applies here"
        );
    }
}
