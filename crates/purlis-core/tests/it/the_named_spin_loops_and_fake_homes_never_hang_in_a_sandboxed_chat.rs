// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The rest of R-355** (T1-47, the T1-46 review's follow-up): purlis-core's tests never hang
//! in a sandboxed chat.
//!
//! - **No fake home under `/home`.** On macOS `/home` is an autofs mount, where every name that
//!   is not there costs a trip to the automounter; a sandbox start resolves hundreds of them
//!   under the machine's home (T1-46). A fake home is `/nonexistent-home/...`.
//! - **No spin loop without a way out.** A test that spins until another thread does something
//!   ends the moment that thread has ended (`is_finished`) or at a deadline, and fails with what
//!   it got, instead of spinning for ever on a run the sandbox refused.
//!
//! What each test asserts on CI is unchanged: only how a test that would wait for ever ends.

use std::path::Path;

/// The files whose spin loops the T1-46 review named.
const SPINNING: [&str; 3] = [
    "src/secrets/ask/tests.rs",
    "src/secrets/ask/run/tests.rs",
    "src/hookwire/spool/tests.rs",
];

fn crate_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files(dir: &Path, into: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("a source folder") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            rust_files(&path, into);
        } else if path.extension().is_some_and(|e| e == "rs") {
            into.push(path);
        }
    }
}

/// Every spin loop in `text` (a loop whose body yields), with its first line's number, that
/// has no way out but what it waits for: nothing in its body looks at whether its thread ended
/// (`is_finished`), at a deadline, or fails (`panic!`).
fn unbounded_spins(text: &str) -> Vec<usize> {
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        if !line.contains("yield_now()") {
            continue;
        }
        // The loop it yields in: the nearest line above that opens one.
        let Some(start) = (0..at).rev().find(|&i| {
            let l = lines[i].trim_start();
            l.starts_with("loop {")
                || l.starts_with("while ")
                || l.contains("= loop {")
                || l.ends_with(" loop {")
        }) else {
            continue;
        };
        let body = lines[start..=at].join("\n");
        if !["is_finished", "deadline", "panic!("]
            .iter()
            .any(|way_out| body.contains(way_out))
        {
            found.push(start + 1);
        }
    }
    found
}

#[test]
fn the_named_spin_loops_and_fake_homes_never_hang_in_a_sandboxed_chat() {
    let mut files = Vec::new();
    rust_files(&crate_root().join("src"), &mut files);
    rust_files(&crate_root().join("tests"), &mut files);
    let this = file!().rsplit('/').next().unwrap_or_default();

    let mut homes = Vec::new();
    for file in &files {
        if file.file_name().is_some_and(|n| n == this) {
            continue;
        }
        let text = std::fs::read_to_string(file).expect("a source file");
        for (n, line) in text.lines().enumerate() {
            // The sandbox tests' operator, and any machine's fake home.
            let fake = line.contains("/home/op")
                || (line.contains("home: Some(") && line.contains("\"/home/"));
            if fake {
                homes.push(format!(
                    "{}:{}: {}",
                    file.strip_prefix(crate_root()).unwrap_or(file).display(),
                    n + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        homes.is_empty(),
        "a fake home under /home, which macOS's automounter makes slow; use /nonexistent-home:\n{}",
        homes.join("\n")
    );

    let mut spins = Vec::new();
    for file in SPINNING {
        let text = std::fs::read_to_string(crate_root().join(file)).expect("a named file");
        spins.extend(
            unbounded_spins(&text)
                .into_iter()
                .map(|line| format!("{file}:{line}")),
        );
    }
    assert!(
        spins.is_empty(),
        "a spin loop that ends only when what it waits for comes: end it when its thread ends \
         (`is_finished()`) or at a deadline, failing with what it got:\n{}",
        spins.join("\n")
    );
}

#[test]
fn a_spin_loop_with_no_way_out_is_found_and_one_with_its_threads_end_is_not() {
    let spinning = "let id = loop {\n    if let Some(x) = look() { break x; }\n    std::thread::yield_now();\n};";
    assert_eq!(unbounded_spins(spinning), vec![1]);
    let bounded = "let id = loop {\n    if let Some(x) = look() { break x; }\n    if run.is_finished() { panic!() }\n    std::thread::yield_now();\n};";
    assert!(unbounded_spins(bounded).is_empty());
    let timed = "while !done() {\n    assert!(Instant::now() < deadline);\n    std::thread::yield_now();\n}";
    assert!(unbounded_spins(timed).is_empty());
}
