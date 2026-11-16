// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every in-process duration the sandbox depends on is a field of one `Timings` value**
//! (T1.1-01, #6848): `purlis_core::sandbox::timings`.
//!
//! This reads the sandbox's code (`sandbox/`, `sandbox.rs` and `sandboxblock.rs` in
//! purlis-core) and fails on any `Duration::from_*` outside `sandbox/timings.rs` that
//! [`ALLOWED`] does not name with its reason: a duration out of process, which runs at real time
//! in the suite, or one in process that is deliberately left unscaled. It reads every Rust file
//! in the workspace, `app/src-tauri` included, and fails on any use of the constants `Timings`
//! replaced. Test code is exempt: a `*tests*` file, a `tests/` folder, and an item under
//! `#[cfg(test)]`.

use std::path::{Path, PathBuf};

/// One duration left outside `Timings` on purpose: its file, the code on its line, and why.
struct Allowed {
    file: &'static str,
    line: &'static str,
    why: &'static str,
}

/// **The durations in the sandbox's code that are not `Timings`'**, each with its reason.
const ALLOWED: &[Allowed] = &[
    // Each is out of process (a program's or git's bound), another component's (the browser
    // door's, a confined extension's, the resolver's), or in process but deliberately unscaled
    // (a retry, a poll, a clamp, a cache, a teardown or a wake on loopback), as its `why` says.
    Allowed {
        file: "sandbox/egress.rs",
        line: "const BACKOFF: Duration = Duration::from_millis(100);",
        why: "a failing accept's retry: no cell orders anything against it",
    },
    Allowed {
        file: "sandbox/egress.rs",
        line: "let _ = TcpStream::connect_timeout(addr, Duration::from_secs(1));",
        why: "the stopping proxy's wake of its own accept, on loopback",
    },
    Allowed {
        file: "sandbox/egress.rs",
        line: "std::thread::sleep(Duration::from_millis(10));",
        why: "the poll while the teller finishes, bounded by `teller_patience`",
    },
    Allowed {
        file: "sandbox/egress.rs",
        line: ".min(Duration::from_secs(5))",
        why: "the pump's clamp on how often it looks at `idle`: a granularity, not a wait",
    },
    Allowed {
        file: "sandbox/egress.rs",
        line: ".max(Duration::from_millis(50));",
        why: "the pump's clamp on how often it looks at `idle`: a granularity, not a wait",
    },
    Allowed {
        file: "sandbox/egress/providers.rs",
        line: "pub const ANSWERS_STAND: Duration = Duration::from_secs(30);",
        why: "a confined extension's cache of a model provider's addresses, not a chat's proxy",
    },
    Allowed {
        file: "sandbox/egress/providers.rs",
        line: "pub const REFUSED_FOR: Duration = Duration::from_secs(10 * 60);",
        why: "a confined extension's cache of a model provider's addresses, not a chat's proxy",
    },
    Allowed {
        file: "sandbox/browser_reach.rs",
        line: "pub const LOOKUP_PATIENCE: Duration = Duration::from_secs(10);",
        why: "the browser's name lookups: the resolver seam (ticket 2) owns lookups",
    },
    Allowed {
        file: "sandbox/browser_reach.rs",
        line: "const BACKOFF: Duration = Duration::from_millis(100);",
        why: "a failing accept's retry: no cell orders anything against it",
    },
    Allowed {
        file: "sandbox/browser_reach.rs",
        line: "const STOP_PATIENCE: Duration = Duration::from_secs(2);",
        why: "how long a stopping browser lane waits for its accept to end: teardown only",
    },
    Allowed {
        file: "sandbox/browser_reach.rs",
        line: "let _ = TcpStream::connect_timeout(&self.address, Duration::from_millis(200));",
        why: "the stopping lane's wake of its own accept, on loopback",
    },
    Allowed {
        file: "sandbox/browser_reach.rs",
        line: "std::thread::sleep(Duration::from_millis(10));",
        why: "the poll while the accept ends, bounded by STOP_PATIENCE",
    },
    Allowed {
        file: "sandbox/door.rs",
        line: "pub const SLOT_WITHIN: Duration = Duration::from_secs(3);",
        why: "the browser door's bound on taking a slot",
    },
    Allowed {
        file: "sandbox/door.rs",
        line: "pub const GO_ANSWERED_WITHIN: Duration = Duration::from_secs(1);",
        why: "the browser door's bound on its go answer",
    },
    Allowed {
        file: "sandbox/door.rs",
        line: "let wait = Duration::from_millis(300);",
        why: "how long a probe of a loopback port waits: whether anything listens there",
    },
    Allowed {
        file: "sandbox/door.rs",
        line: "let _ = TcpStream::connect_timeout(&at, Duration::from_millis(200));",
        why: "the stopping door's wake of its own accept, on loopback",
    },
    Allowed {
        file: "sandbox/tunnel.rs",
        line: "const IDLE: Duration = Duration::from_secs(3600);",
        why: "an ssh route's idle bound, an hour: no cell waits it out",
    },
    Allowed {
        file: "sandbox/tunnel.rs",
        line: "const BACKOFF: Duration = Duration::from_millis(100);",
        why: "a failing accept's retry: no cell orders anything against it",
    },
    Allowed {
        file: "sandbox/tunnel.rs",
        line: "let _ = TcpStream::connect_timeout(&self.addr, Duration::from_secs(1));",
        why: "the ending route's wake of its own accept, on loopback",
    },
    Allowed {
        file: "sandbox/sshproxy.rs",
        line: "const GREETING: Duration = Duration::from_secs(10);",
        why: "how long ssh's proxy command waits for the SOCKS greeting: ssh's, out of process",
    },
    Allowed {
        file: "sandbox/program.rs",
        line: "pub const PATIENCE: Duration = Duration::from_secs(10);",
        why: "how long a harness's version probe may run: a program out of process",
    },
    Allowed {
        file: "sandbox/program.rs",
        line: "std::thread::sleep(Duration::from_millis(10));",
        why: "the poll while a version probe runs, bounded by its PATIENCE",
    },
    Allowed {
        file: "sandbox/program.rs",
        line: ".recv_timeout(left.max(Duration::from_millis(100)))",
        why: "the floor on reading a finished probe's answer",
    },
    Allowed {
        file: "sandbox/team.rs",
        line: "const GIT_WAIT: Duration = Duration::from_secs(5);",
        why: "a git command's bound while the team's policy is read: git, out of process",
    },
    Allowed {
        file: "sandbox/protection.rs",
        line: "const GIT_WAIT: std::time::Duration = std::time::Duration::from_secs(2);",
        why: "a git command's bound while a protection level is read: git, out of process",
    },
    Allowed {
        file: "sandbox/arrived.rs",
        line: "pub const BEGIN_WAITS_AT_MOST: Duration = Duration::from_millis(250);",
        why: "a bound on the arrival checks' own work, not a timer a cell orders",
    },
    Allowed {
        file: "sandbox/arrived.rs",
        line: "pub const CHECKS_TAKE_AT_MOST: Duration = Duration::from_secs(30);",
        why: "a bound on the arrival checks' own work, not a timer a cell orders",
    },
    Allowed {
        file: "sandbox/arrived.rs",
        line: "let at = Duration::new(",
        why: "a file's change time read as a point in time since the epoch: not a wait",
    },
    Allowed {
        file: "sandbox/nat64.rs",
        line: "const READ_AGAIN_AFTER: std::time::Duration = std::time::Duration::from_secs(300);",
        why: "how long a NAT64 prefix stands: the test build swaps NAT64 (ticket 6's inventory)",
    },
    Allowed {
        file: "sandbox/nat64.rs",
        line: "const ASK_AGAIN_AFTER_A_FAILURE: std::time::Duration = std::time::Duration::from_secs(30);",
        why: "how soon a failed NAT64 lookup is tried again: the test build swaps NAT64",
    },
    Allowed {
        file: "sandbox/personal/no_sandbox.rs",
        line: "const BLAME_WITHIN: Duration = Duration::from_secs(3);",
        why: "git blame's bound, out of process, on naming who added a vault: it only picks \
              the refusal's wording (a name or \"your team\"), never a wait of the sandbox's",
    },
    Allowed {
        file: "sandbox.rs",
        line: "const NAMES_KEPT_FOR: std::time::Duration = std::time::Duration::from_secs(60);",
        why: "how long a look through the project's names is cached: no cell orders against it",
    },
    Allowed {
        file: "sandbox.rs",
        line: "const MACHINE_HELD_KEPT: std::time::Duration = std::time::Duration::from_secs(5);",
        why: "how long a read of the PATH folders is cached: no cell orders against it",
    },
];

/// **The constants `Timings` replaced**, by the path they were read by, and by name in the
/// files that held or read them.
const OLD: &[&str] = &[
    "HOLD",
    "GROUP",
    "MOST_HELD_OPEN",
    "THROTTLE_WINDOW",
    "PER_CHAT",
    "PATIENCE",
    "TELLER_PATIENCE",
    "LIMITS",
    "TALLY_WINDOW",
    "TALLY_TICK",
];

/// The paths the old constants were read by from anywhere.
const OLD_PATHS: &[&str] = &[
    "asks::HOLD",
    "asks::GROUP",
    "asks::MOST_HELD_OPEN",
    "sandboxblock::THROTTLE_WINDOW",
    "Throttle::PER_CHAT",
    "egress::PATIENCE",
    "egress::TELLER_PATIENCE",
    "egress::LIMITS",
    "egress::TALLY_WINDOW",
    "egress::TALLY_TICK",
];

/// The modules whose `use` of a bare old name is one of the old constants.
const OLD_MODULES: &[&str] = &["asks::", "egress::", "sandboxblock::", "Throttle::"];

/// The files that held or read the old constants by their bare names.
const CONSUMERS: &[&str] = &[
    "crates/purlis-core/src/sandbox/asks.rs",
    "crates/purlis-core/src/sandbox/egress.rs",
    "crates/purlis-core/src/sandbox/browser_reach.rs",
    "crates/purlis-core/src/sandbox/plan.rs",
    "crates/purlis-core/src/sandbox/hosted.rs",
    "crates/purlis-core/src/sandboxblock.rs",
    "app/src-tauri/src/asking.rs",
    "app/src-tauri/src/hooks.rs",
];

fn rust_files(dir: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "target" || name == "node_modules" || name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            rust_files(&path, into);
        } else if name.ends_with(".rs") {
            into.push(path);
        }
    }
}

/// Whether `at`, a path from the workspace's root, is test code by its name.
fn a_test_file(at: &str) -> bool {
    let name = at.rsplit('/').next().unwrap_or(at);
    name.contains("tests")
        || at
            .split('/')
            .any(|part| part == "tests" || part == "benches")
}

/// The lines of `text` that are not test code, each with its number, comments left out: an
/// item under `#[cfg(test)]` (or `#[cfg(any(test, ...))]`) is skipped to the end of its braces,
/// or to its `;`.
fn code_lines(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut lines = text.lines().enumerate();
    while let Some((n, line)) = lines.next() {
        let code = line.trim_start();
        if code.starts_with("#[cfg(test)]") || code.starts_with("#[cfg(any(test") {
            let mut depth = 0usize;
            let mut opened = false;
            let mut open = None;
            for (_, next) in lines.by_ref() {
                let (more, fewer) = braces(next, &mut open);
                opened |= more > 0;
                depth = (depth + more).saturating_sub(fewer);
                let next = next.split("//").next().unwrap_or_default();
                if (opened && depth == 0) || (!opened && next.trim_end().ends_with(';')) {
                    break;
                }
            }
            continue;
        }
        if code.starts_with("//") {
            continue;
        }
        out.push((n + 1, line));
    }
    out
}

/// Whether `line` builds a `Duration`.
fn builds_a_duration(line: &str) -> bool {
    line.contains("Duration::from_") || line.contains("Duration::new(")
}

/// A string literal left open: a plain one, or a raw one closed by `"` and this many `#`.
#[derive(Clone, Copy)]
enum Open {
    Plain,
    Raw(usize),
}

/// How `line` moves the brace depth, as `(opened, closed)`, its string and char literals and its
/// comment left out. `open` carries a string left open at the end of a line to the next.
fn braces(line: &str, open: &mut Option<Open>) -> (usize, usize) {
    let chars: Vec<char> = line.chars().collect();
    let (mut opened, mut closed) = (0, 0);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match *open {
            Some(Open::Plain) => match c {
                '\\' => i += 1,
                '"' => *open = None,
                _ => {}
            },
            Some(Open::Raw(hashes)) => {
                if c == '"' && (1..=hashes).all(|n| chars.get(i + n) == Some(&'#')) {
                    *open = None;
                    i += hashes;
                }
            }
            None => match c {
                '/' if chars.get(i + 1) == Some(&'/') => break,
                '"' => *open = Some(Open::Plain),
                // A raw string, `r"…"` or `r#"…"#`, unless `r` ends a longer name.
                'r' if !i
                    .checked_sub(1)
                    .and_then(|before| chars.get(before))
                    .is_some_and(|b| b.is_ascii_alphanumeric() || *b == '_') =>
                {
                    let hashes = chars[i + 1..].iter().take_while(|h| **h == '#').count();
                    if chars.get(i + 1 + hashes) == Some(&'"') {
                        *open = Some(Open::Raw(hashes));
                        i += 1 + hashes;
                    }
                }
                // A char literal, `'{'` or `'\\u{7b}'`; a lifetime has no closing quote.
                '\'' if chars.get(i + 1) == Some(&'\\') => {
                    i += 2;
                    while i < chars.len() && chars[i] != '\'' {
                        i += 1;
                    }
                }
                '\'' if chars.get(i + 2) == Some(&'\'') => i += 2,
                '{' => opened += 1,
                '}' => closed += 1,
                _ => {}
            },
        }
        i += 1;
    }
    (opened, closed)
}

/// Whether `word` stands alone in `line`, not as part of a longer name.
fn has_word(line: &str, word: &str) -> bool {
    let named = |c: char| c.is_ascii_alphanumeric() || c == '_';
    line.match_indices(word).any(|(at, _)| {
        let before = line[..at].chars().next_back();
        let after = line[at + word.len()..].chars().next();
        !before.is_some_and(named) && !after.is_some_and(named)
    })
}

/// Each `use` statement of `lines`, whole, with the line it starts on.
fn uses(lines: &[(usize, &str)]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut open: Option<(usize, String)> = None;
    for (n, line) in lines {
        let code = line.trim();
        if let Some((_, ref mut stmt)) = open {
            stmt.push(' ');
            stmt.push_str(code);
        } else if code.starts_with("use ") || code.starts_with("pub use ") {
            open = Some((*n, code.to_owned()));
        }
        if open.as_ref().is_some_and(|(_, stmt)| stmt.ends_with(';')) {
            out.extend(open.take());
        }
    }
    out
}

#[test]
fn no_sandbox_duration_lives_outside_timings() {
    purlis_core::unsteered!();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    rust_files(&root.join("crates"), &mut files);
    rust_files(&root.join("app/src-tauri"), &mut files);
    assert!(files.len() > 50, "the walk found {} files", files.len());

    let core = "crates/purlis-core/src/";
    let this = Path::new(file!()).file_name().expect("this file's name");
    let mut found = Vec::new();
    let mut used = vec![false; ALLOWED.len()];
    let mut timings_read = false;
    for file in &files {
        if file.file_name() == Some(this) {
            continue;
        }
        let at = file
            .strip_prefix(&root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        let at = at.trim_start_matches("./").to_owned();
        if a_test_file(&at) {
            continue;
        }
        let text = std::fs::read_to_string(file).expect("a Rust file");
        let lines = code_lines(&text);
        let in_core = at.strip_prefix(core);
        let sandboxed = in_core.is_some_and(|rest| {
            rest.starts_with("sandbox/") || rest == "sandbox.rs" || rest == "sandboxblock.rs"
        });

        if in_core == Some("sandbox/timings.rs") {
            timings_read = true;
            // Production is not configurable: nothing here reads the environment or settings.
            for word in ["std::env", "env::var", "serde", "settings"] {
                assert!(
                    !lines.iter().any(|(_, line)| line.contains(word)),
                    "Timings reads `{word}`: production's timings are not configurable"
                );
            }
            continue;
        }

        for (n, line) in &lines {
            // A duration in the sandbox's code is `Timings`', or allowed by name.
            if sandboxed && builds_a_duration(line) {
                let rest = in_core.unwrap_or_default();
                match ALLOWED
                    .iter()
                    .position(|one| one.file == rest && line.trim() == one.line)
                {
                    Some(which) => used[which] = true,
                    None => found.push(format!(
                        "{at}:{n}: a duration outside Timings: {}",
                        line.trim()
                    )),
                }
            }
            // The old constants, by the paths they were read by.
            for path in OLD_PATHS {
                if line.contains(path) {
                    found.push(format!("{at}:{n}: reads `{path}`: {}", line.trim()));
                }
            }
            // By their bare names, in the files that held or read them.
            if CONSUMERS.contains(&at.as_str()) {
                for word in OLD {
                    if has_word(line, word) {
                        found.push(format!("{at}:{n}: names `{word}`: {}", line.trim()));
                    }
                }
            }
        }
        // And by a `use` of their module, anywhere.
        for (n, stmt) in uses(&lines) {
            if OLD_MODULES.iter().any(|module| stmt.contains(module)) {
                for word in OLD {
                    if stmt
                        .split(['{', '}', ',', ' ', ';'])
                        .any(|part| part == *word || part.ends_with(&format!("::{word}")))
                    {
                        found.push(format!("{at}:{n}: imports `{word}`: {stmt}"));
                    }
                }
            }
        }
    }

    assert!(timings_read, "sandbox/timings.rs was read");
    assert!(
        found.is_empty(),
        "every in-process duration the sandbox depends on is a field of Timings \
         (crates/purlis-core/src/sandbox/timings.rs); one that is not names its reason in \
         ALLOWED:\n{}",
        found.join("\n")
    );
    let stale: Vec<String> = ALLOWED
        .iter()
        .zip(&used)
        .filter(|(_, used)| !**used)
        .map(|(one, _)| format!("{}: {} ({})", one.file, one.line, one.why))
        .collect();
    assert!(
        stale.is_empty(),
        "an allowed duration is no longer in its file; take it out of ALLOWED:\n{}",
        stale.join("\n")
    );
}

#[test]
fn a_duration_built_with_new_is_a_duration_too() {
    assert!(builds_a_duration(
        "const WAIT: Duration = Duration::new(5, 0);"
    ));
    assert!(builds_a_duration(
        "let d = std::time::Duration::from_secs(5);"
    ));
    assert!(!builds_a_duration("let d = timings.hold();"));
}

#[test]
fn a_brace_in_a_string_or_char_does_not_end_or_extend_a_test_items_skip() {
    let text = "\
#[cfg(test)]
mod tests {
    const CLOSE: &str = \"}\";
    const LATE: Duration = Duration::from_secs(9);
    const QUOTED: &str = \"a \\\" { quote\";
    const RAW: &str = r#\"{\"a\": {\"b\": 1}\"#;
    const RAW_OPEN: &str = r#\"{\"a\": \"}\"#;
}
const KEPT: Duration = Duration::from_secs(1);
#[cfg(test)]
fn opens() -> char {
    '{'
}
const ALSO_KEPT: Duration = Duration::from_secs(2);
";
    let kept: Vec<&str> = code_lines(text).into_iter().map(|(_, line)| line).collect();
    assert_eq!(
        kept,
        [
            "const KEPT: Duration = Duration::from_secs(1);",
            "const ALSO_KEPT: Duration = Duration::from_secs(2);",
        ]
    );
}
