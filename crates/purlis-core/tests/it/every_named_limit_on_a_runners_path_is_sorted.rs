// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every named limit on a runner's path has a group** (#4288, spec #4275): whether the right
//! value depends on the person's setup, and so is a pool's limit, or protects a boundary, or is
//! purlis's own tuning. So a new number has an obvious home, and nobody has to sort it twice.
//!
//! The scan reads numeric `const`s outside test code in [`TREES`]. It can't see a bare literal
//! such as `from_secs(10)` used inline, or a limit outside those files. The list goes both ways:
//! an entry whose const has gone fails too.

use std::path::{Path, PathBuf};

use Group::{AlreadyALimit, Candidate, Fixed, Internal, PoolLimit, Security};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    /// A default in `purlis_core::poollimits`.
    PoolLimit,
    /// A bound on a pool setting that was a limit before that list.
    AlreadyALimit,
    /// In `purlis_session_protocol::fixedlimits`, or the top of what a limit can be.
    Fixed,
    /// Protects a boundary: never raised.
    Security,
    /// purlis's own tuning, or what a format or tool holds.
    Internal,
    /// Could be a pool limit: for the person to rule on (#4288).
    Candidate,
}

const TREES: &[&str] = &[
    "crates/purlis-core/src/runnerlink.rs",
    "crates/purlis-core/src/dockerpool.rs",
    "crates/purlis-core/src/dockerpool",
    "crates/purlis-core/src/clusterpool.rs",
    "crates/purlis-core/src/clusterpool",
    "crates/purlis-core/src/runners.rs",
    "crates/purlis-core/src/pools.rs",
    "crates/purlis-core/src/boundedrun.rs",
    "crates/purlis-core/src/profilestore/onrunner.rs",
    "crates/purlis-core/src/profilestore/sign_in",
    "app/src-tauri/src/hostedsessions.rs",
    "app/src-tauri/src/runnerlinks.rs",
    "app/src-tauri/src/poolprofiles.rs",
    "app/src-tauri/src/runnercode.rs",
    "crates/session-protocol/src/code.rs",
    "crates/session-protocol/src/link.rs",
    "crates/session-protocol/src/session.rs",
    "crates/host/src/reach.rs",
    "crates/host/src/relay.rs",
];

/// Every one: its file, its name, its group, and why.
const SORTED: &[(&str, &str, Group, &str)] = &[
    (
        "app/src-tauri/src/hostedsessions.rs",
        "LINK_WITHIN",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "app/src-tauri/src/hostedsessions.rs",
        "ANSWER_WITHIN",
        Internal,
        "the wait on this machine's own host: a pool's follows answerWait",
    ),
    (
        "app/src-tauri/src/hostedsessions.rs",
        "LOOKS_FOR_A_CANCEL_EVERY",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "app/src-tauri/src/hostedsessions.rs",
        "UNTIL_THE_HOST_SAYS",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "app/src-tauri/src/hostedsessions.rs",
        "MOST_HELD_FRAME_BYTES",
        Security,
        "bounds what a runner or a peer can make this machine hold",
    ),
    (
        "app/src-tauri/src/hostedsessions.rs",
        "MOST_HELD_FRAME_HEIGHT",
        Security,
        "bounds what a runner or a peer can make this machine hold",
    ),
    (
        "app/src-tauri/src/hostedsessions.rs",
        "A_REPLY_IS_AT_MOST",
        Internal,
        "purlis's own tuning",
    ),
    (
        "app/src-tauri/src/hostedsessions.rs",
        "MOST_UNCLAIMED_ENDS",
        Internal,
        "purlis's own tuning",
    ),
    (
        "app/src-tauri/src/hostedsessions.rs",
        "MIB",
        Internal,
        "purlis's own tuning",
    ),
    (
        "app/src-tauri/src/hostedsessions.rs",
        "NAMED_AGAIN_AFTER",
        Internal,
        "purlis's own tuning",
    ),
    (
        "app/src-tauri/src/hostedsessions.rs",
        "LOOK_FOR_A_NEW_HOST_EVERY",
        Internal,
        "purlis's own tuning",
    ),
    (
        "app/src-tauri/src/poolprofiles.rs",
        "STEP_WITHIN",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "app/src-tauri/src/runnerlinks.rs",
        "MOST_HELD_PAGES",
        Internal,
        "purlis's own tuning",
    ),
    (
        "app/src-tauri/src/runnerlinks.rs",
        "ANSWER_WITHIN",
        Internal,
        "what a link waits with no pool's limit to hand (#4275's leftovers)",
    ),
    (
        "app/src-tauri/src/runnerlinks.rs",
        "KEPT_WITHIN",
        Internal,
        "purlis's own tuning",
    ),
    (
        "app/src-tauri/src/runnerlinks.rs",
        "ALL_KEPT_WITHIN",
        Internal,
        "purlis's own tuning",
    ),
    (
        "app/src-tauri/src/runnerlinks.rs",
        "SECONDS_KEPT_PER_MIB",
        Internal,
        "purlis's own tuning",
    ),
    (
        "crates/host/src/reach.rs",
        "REACHED_WITHIN",
        Fixed,
        "fixedlimits::STOP_WAIT",
    ),
    (
        "crates/host/src/reach.rs",
        "TRIED_AGAIN_EVERY",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/host/src/reach.rs",
        "MOST_KEPT_BYTES",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/host/src/relay.rs",
        "MOST_PUSHED_QUEUED",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/host/src/relay.rs",
        "PARKED_FOR",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/boundedrun.rs",
        "SAID_AT_LEAST",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/boundedrun.rs",
        "LOOKED_AT_EVERY",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/clusterpool.rs",
        "MOST_AHEAD",
        Security,
        "bounds what signed or untrusted input can claim",
    ),
    (
        "crates/purlis-core/src/clusterpool.rs",
        "MOST_VALUE",
        Security,
        "bounds what signed or untrusted input can claim",
    ),
    (
        "crates/purlis-core/src/clusterpool.rs",
        "ROLL_MOST_AHEAD",
        Security,
        "bounds what signed or untrusted input can claim",
    ),
    (
        "crates/purlis-core/src/clusterpool.rs",
        "MOST_PRINTED",
        Fixed,
        "fixedlimits::RUNNER_OUTPUT",
    ),
    (
        "crates/purlis-core/src/clusterpool.rs",
        "MOST_SAID",
        Fixed,
        "fixedlimits::RUNNER_OUTPUT",
    ),
    (
        "crates/purlis-core/src/clusterpool.rs",
        "CLAIM_WRITES",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/clusterpool.rs",
        "MINT_READS",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/clusterpool.rs",
        "MOST_CLAIM_REPOS",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/purlis-core/src/clusterpool/idle.rs",
        "PARKED_FOR",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/dockerpool.rs",
        "CARRIED_WITHIN",
        Candidate,
        "depends on the setup: for the person",
    ),
    (
        "crates/purlis-core/src/dockerpool.rs",
        "MOST_NAME",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/purlis-core/src/dockerpool.rs",
        "FEWEST_PIDS",
        AlreadyALimit,
        "the floor of a pool's processes, CPUs or memory",
    ),
    (
        "crates/purlis-core/src/dockerpool.rs",
        "FEWEST_MILLICPUS",
        AlreadyALimit,
        "the floor of a pool's processes, CPUs or memory",
    ),
    (
        "crates/purlis-core/src/dockerpool.rs",
        "LEAST_MEMORY_MIB",
        AlreadyALimit,
        "the floor of a pool's processes, CPUs or memory",
    ),
    (
        "crates/purlis-core/src/dockerpool.rs",
        "SUSPEND_AFTER",
        AlreadyALimit,
        "the pool's suspendAfter",
    ),
    (
        "crates/purlis-core/src/dockerpool.rs",
        "LEAST_SUSPEND_AFTER",
        AlreadyALimit,
        "the pool's suspendAfter",
    ),
    (
        "crates/purlis-core/src/dockerpool.rs",
        "MOST_SUSPEND_AFTER",
        AlreadyALimit,
        "the pool's suspendAfter",
    ),
    (
        "crates/purlis-core/src/dockerpool.rs",
        "STANDING_MOST",
        Fixed,
        "fixedlimits::RUNNER_OUTPUT",
    ),
    (
        "crates/purlis-core/src/dockerpool.rs",
        "STANDING_WITHIN",
        Candidate,
        "depends on the setup: for the person",
    ),
    (
        "crates/purlis-core/src/dockerpool.rs",
        "READS_WITHIN",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/dockerpool.rs",
        "MOST_INSPECTED",
        Fixed,
        "fixedlimits::RUNNER_OUTPUT",
    ),
    (
        "crates/purlis-core/src/dockerpool.rs",
        "PROBE_WITHIN",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/dockerpool.rs",
        "SIGN_OUT_WITHIN",
        Candidate,
        "depends on the setup: for the person",
    ),
    (
        "crates/purlis-core/src/dockerpool.rs",
        "MOST_SAID",
        Fixed,
        "fixedlimits::RUNNER_OUTPUT",
    ),
    (
        "crates/purlis-core/src/dockerpool/settings.rs",
        "MOST_READ",
        Security,
        "bounds what signed or untrusted input can claim",
    ),
    (
        "crates/purlis-core/src/dockerpool/settings.rs",
        "STALE_AFTER",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/pools.rs",
        "CLOCKS_APART",
        Security,
        "bounds what signed or untrusted input can claim",
    ),
    (
        "crates/purlis-core/src/pools.rs",
        "VOUCHED_WITHIN",
        PoolLimit,
        "a default in poollimits",
    ),
    (
        "crates/purlis-core/src/pools.rs",
        "ASKED_EVERY",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/pools.rs",
        "REPOS_WAITED_FOR",
        Candidate,
        "depends on the setup: for the person",
    ),
    (
        "crates/purlis-core/src/pools.rs",
        "VOUCHED_NOW_WITHIN",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/profilestore/onrunner.rs",
        "MOST_SETTINGS_BYTES",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/purlis-core/src/profilestore/onrunner.rs",
        "MOST_SETTINGS_FILES",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/purlis-core/src/profilestore/onrunner.rs",
        "FRAMING_BYTES",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/purlis-core/src/profilestore/onrunner.rs",
        "REWRITING_ROOM",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/purlis-core/src/profilestore/onrunner.rs",
        "MIB",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/purlis-core/src/profilestore/onrunner.rs",
        "MOST_REMOVED_BYTES",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/callback.rs",
        "PORT",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/callback.rs",
        "MOST_TARGET",
        Security,
        "bounds what signed or untrusted input can claim",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/callback.rs",
        "MOST_HEAD",
        Security,
        "bounds what signed or untrusted input can claim",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/callback.rs",
        "ASKED_WITHIN",
        Security,
        "bounds what signed or untrusted input can claim",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/callback.rs",
        "MOST_READ_AT_ONCE",
        Security,
        "bounds what signed or untrusted input can claim",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/callback.rs",
        "LOOKED_EVERY",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/callback.rs",
        "REACHED_WITHIN",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/callback.rs",
        "ANSWERED_WITHIN",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/callback.rs",
        "CARRIED_WITHIN",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/driven.rs",
        "PAGE_WITHIN",
        Candidate,
        "depends on the setup: for the person",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/driven.rs",
        "LONGEST",
        PoolLimit,
        "a default in poollimits",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/driven.rs",
        "UNDRIVEN_AFTER",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/driven.rs",
        "MOST_CODE",
        Security,
        "bounds what a login's output or a paste can make purlis hold",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/driven.rs",
        "DRAIN_WITHIN",
        Internal,
        "purlis's own tuning",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/driven.rs",
        "KEPT_LINES",
        Internal,
        "purlis's own tuning",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/driven.rs",
        "MOST_LINE",
        Internal,
        "purlis's own tuning",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/driven.rs",
        "MOST_PARTIAL",
        Security,
        "bounds what a login's output or a paste can make purlis hold",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/driven.rs",
        "MOST_SAID",
        Internal,
        "purlis's own tuning",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/driven.rs",
        "NEXT_WITHIN",
        Internal,
        "purlis's own tuning",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/driven.rs",
        "MOST_TYPED",
        Security,
        "bounds what a login's output or a paste can make purlis hold",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/driven.rs",
        "MOST_WAITING",
        Security,
        "bounds what a login's output or a paste can make purlis hold",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/driven.rs",
        "UNHEARD_WITHIN",
        Internal,
        "purlis's own tuning",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/driven.rs",
        "LEAST_STATE",
        Security,
        "bounds what a login's output or a paste can make purlis hold",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/driven.rs",
        "MOST_STATE",
        Security,
        "bounds what a login's output or a paste can make purlis hold",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/held.rs",
        "LOOK",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/held.rs",
        "ENDS_WITHIN",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/held.rs",
        "SEEN_WITHIN",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/terminal.rs",
        "LOOK_EVERY",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/profilestore/sign_in/terminal.rs",
        "STILL_EVERY",
        Internal,
        "how purlis waits or looks",
    ),
    (
        "crates/purlis-core/src/runnerlink.rs",
        "MOST_LINE",
        Fixed,
        "fixedlimits::RUNNER_OUTPUT",
    ),
    (
        "crates/purlis-core/src/runners.rs",
        "MOST_NAME",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/session-protocol/src/code.rs",
        "CODE",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/session-protocol/src/code.rs",
        "MOST_BUNDLE_BYTES",
        PoolLimit,
        "a default in poollimits",
    ),
    (
        "crates/session-protocol/src/code.rs",
        "HIGHEST_BUNDLE_BYTES",
        Fixed,
        "the top of what a limit can be",
    ),
    (
        "crates/session-protocol/src/code.rs",
        "LONGEST_TRANSFER_SECS",
        Fixed,
        "the top of what a limit can be",
    ),
    (
        "crates/session-protocol/src/code.rs",
        "LEN",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/session-protocol/src/link.rs",
        "MOST_CONTROL_FRAME_BYTES",
        Security,
        "bounds what a runner or a peer can make this machine hold",
    ),
    (
        "crates/session-protocol/src/link.rs",
        "MOST_UNACCEPTED_STREAMS",
        Security,
        "bounds what signed or untrusted input can claim",
    ),
    (
        "crates/session-protocol/src/link.rs",
        "CONTROL_LANE",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/session-protocol/src/link.rs",
        "MOST_STREAMS",
        Security,
        "bounds what a runner or a peer can make this machine hold",
    ),
    (
        "crates/session-protocol/src/session.rs",
        "MOST",
        Security,
        "bounds what a runner or a peer can make this machine hold",
    ),
    (
        "crates/session-protocol/src/session.rs",
        "MOST_PROFILE_SETTINGS_BYTES",
        PoolLimit,
        "a default in poollimits",
    ),
    (
        "crates/session-protocol/src/session.rs",
        "MOST_PROFILE_SETTINGS_FILES",
        PoolLimit,
        "a default in poollimits",
    ),
    (
        "crates/session-protocol/src/session.rs",
        "MOST_PROFILE_SETTINGS_STAGED_BYTES",
        Fixed,
        "the top of what a limit can be",
    ),
    (
        "crates/session-protocol/src/session.rs",
        "MOST_PROFILE_SETTINGS_STAGED_FILES",
        Fixed,
        "the top of what a limit can be",
    ),
    (
        "crates/session-protocol/src/session.rs",
        "MOST_SETTINGS_PATH_BYTES",
        Security,
        "bounds what a runner or a peer can make this machine hold",
    ),
    (
        "crates/session-protocol/src/session.rs",
        "MOST_BATCH_BYTES",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/session-protocol/src/session.rs",
        "PIECE_FRAMING",
        Internal,
        "what the format or a tool holds",
    ),
    (
        "crates/session-protocol/src/session.rs",
        "BYTES",
        Security,
        "the length of a key",
    ),
    (
        "crates/session-protocol/src/session.rs",
        "MOST_CALLS_IN_FLIGHT",
        Security,
        "bounds what a runner or a peer can make this machine hold",
    ),
    (
        "crates/session-protocol/src/session.rs",
        "MOST_EVENTS_QUEUED",
        Internal,
        "what the format or a tool holds",
    ),
];

fn sources(path: &Path, into: &mut Vec<PathBuf>) {
    if path.is_file() {
        into.push(path.to_path_buf());
        return;
    }
    let entries = std::fs::read_dir(path).unwrap_or_else(|why| panic!("{path:?}: {why}"));
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if path.is_dir() {
            if !name.contains("tests") {
                sources(&path, into);
            }
        } else if name.ends_with(".rs") && !name.contains("tests") {
            into.push(path);
        }
    }
}

/// The name of the numeric const `line` declares, where it declares one.
fn numeric_const(line: &str) -> Option<&str> {
    let line = line.trim_start();
    let line = line.strip_prefix("pub ").unwrap_or(line);
    let line = match line.strip_prefix("pub(") {
        Some(rest) => rest.split_once(") ")?.1,
        None => line,
    };
    let (name, rest) = line.strip_prefix("const ")?.split_once(':')?;
    let upper = name
        .bytes()
        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_');
    let kind = rest.split('=').next()?;
    let numeric = [
        "Duration", "u8", "u16", "u32", "u64", "u128", "usize", "i32", "i64", "isize", "f64",
        "NonZero",
    ]
    .iter()
    .any(|word| kind.contains(word));
    (upper && numeric).then_some(name)
}

/// The numeric consts of `text` a shipped build has: up to a `#[cfg(test)] mod`, and leaving out
/// only the one item `#[cfg(test)]` stands on anywhere else.
fn shipped_consts(text: &str) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().collect();
    let mut consts = Vec::new();
    let mut test_only = false;
    for (at, line) in lines.iter().enumerate() {
        let line = line.trim();
        if line == "#[cfg(test)]" {
            let next = lines.get(at + 1).map_or("", |next| next.trim());
            if ["mod ", "pub mod ", "pub(crate) mod "]
                .iter()
                .any(|start| next.starts_with(start))
            {
                break;
            }
            test_only = true;
        } else if !line.is_empty() && !line.starts_with("//") && !line.starts_with("#[") {
            if !test_only && let Some(constant) = numeric_const(line) {
                consts.push(constant);
            }
            test_only = false;
        }
    }
    consts
}

/// The scan's own edges: a test-only item costs that item, and only a test module ends the file.
#[test]
fn the_scan_reads_past_a_test_only_item_and_stops_at_a_test_module() {
    purlis_core::unsteered!();
    let text = "const A: u8 = 1;\n#[cfg(test)]\nconst B: u8 = 2;\n#[cfg(test)]\nfn c() {}\n\
                pub(crate) const D: Duration = X;\n#[cfg(test)]\nmod tests {\nconst E: u8 = 3;\n}\n";
    assert_eq!(shipped_consts(text), ["A", "D"]);
}

fn found() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for tree in TREES {
        assert!(
            root.join(tree).exists(),
            "{tree} is gone: name where it went"
        );
        sources(&root.join(tree), &mut files);
    }
    assert!(
        files.len() > 15,
        "the walk found only {} files",
        files.len()
    );
    let mut found = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let name = file
            .strip_prefix(&root)
            .unwrap()
            .display()
            .to_string()
            .replace('\\', "/");
        found.extend(
            shipped_consts(&text)
                .into_iter()
                .map(|constant| (name.clone(), constant.to_owned())),
        );
    }
    found
}

#[test]
fn every_named_limit_on_a_runners_path_has_a_group() {
    purlis_core::unsteered!();
    let found = found();
    for (file, name) in &found {
        assert!(
            SORTED.iter().any(|one| one.0 == file && one.1 == name),
            "{file}: {name} has no group here. Sort it by the spec's rule (#4275): does its right \
             value depend on the person's setup, does it protect a boundary, or is it tuning?"
        );
    }
    for (file, name, _, why) in SORTED {
        assert!(!why.is_empty());
        assert!(
            found.iter().any(|one| one.0 == *file && one.1 == *name),
            "{file}: {name} is sorted here and no longer there: take it off"
        );
    }
}

/// What the person is asked to rule on: nothing becomes a limit by being listed.
#[test]
fn the_candidates_are_the_ones_brought_to_the_person() {
    purlis_core::unsteered!();
    let candidates: Vec<&str> = SORTED
        .iter()
        .filter(|one| one.2 == Candidate)
        .map(|one| one.1)
        .collect();
    assert_eq!(
        candidates,
        [
            "CARRIED_WITHIN",
            "STANDING_WITHIN",
            "SIGN_OUT_WITHIN",
            "REPOS_WAITED_FOR",
            "PAGE_WITHIN"
        ]
    );
}
