// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Printed text that quotes a refusal never raises a block (#1972): only a refusal a command
//! may have met does, on every harness's shape of a result.

use super::*;
use serde_json::json;

const ROOT: &str = "/Users/dev/plane";
const CHAT: &str = "/Users/dev/plane/workspaces/alpha/repo";
const SOCKET: &str = "/Users/dev/.docker/run/docker.sock";

fn place() -> Place<'static> {
    Place {
        root: Path::new(ROOT),
        chat: Path::new(CHAT),
        cwd: Path::new(CHAT),
        home: Some(Path::new("/Users/dev")),
    }
}

/// A test file's lines that quote every kind of refusal a command's output is read for: a
/// socket, a lookup, a proxy's, a client's `403`, a service on this Mac and the certificate
/// check, each ending where the refusal's own words do.
fn quoted() -> String {
    [
        format!("        \"dial unix {SOCKET}: connect: operation not permitted\","),
        "    let said = \"error: dial tcp: lookup db.example.com: no such host\";".to_owned(),
        "curl: (6) Could not resolve host: api.example.com".to_owned(),
        "psql: error: could not translate host name \"db.example.com\" to address: nodename nor \
         servname provided, or not known"
            .to_owned(),
        "curl: (56) CONNECT tunnel failed, response 403".to_owned(),
        "Connection blocked by network allowlist".to_owned(),
        "purlis's sandbox does not allow example.org:443: no egress preset or host of this \
         project lists it"
            .to_owned(),
        "dial tcp 127.0.0.1:5432: connect: operation not permitted".to_owned(),
        "tls: failed to verify certificate: x509: OSStatus -26276".to_owned(),
        format!(
            "docker: permission denied while trying to connect to the Docker daemon socket at \
             unix://{SOCKET}: Head \"http://x/_ping\": dial unix {SOCKET}: connect: operation \
             not permitted."
        ),
    ]
    .join("\n")
}

/// Claude Code's `PostToolUse`: a command that did not fail, each stream apart.
fn came_back(command: &str, stdout: &str, stderr: &str) -> serde_json::Value {
    json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "tool_response": {"stdout": stdout, "stderr": stderr, "interrupted": false},
    })
}

/// Codex's `PostToolUse` for every command, and opencode's for one that did not fail: both
/// streams in one string, with no failure beside it.
fn one_string(command: &str, said: &str) -> serde_json::Value {
    json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "tool_response": said,
    })
}

/// Claude Code's `PostToolUseFailure`: the error it gives the model, both streams.
fn failed(command: &str, error: &str) -> serde_json::Value {
    json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "error": error,
    })
}

/// The live case: `sed -n` over a test file that quotes the words raised a socket block and a
/// Notice on the chat, although nothing was refused. A line whose every program only reads or
/// prints raises none from what it handed back, on either stream or as one string; and a
/// background command's output read back, with no line beside it, raises none.
#[test]
fn printing_a_file_that_quotes_refusal_lines_raises_nothing() {
    let text = quoted();
    let output_file = "/private/tmp/claude-501/-Users-dev-plane/0b1c/tasks/b7x2.output";
    for payload in [
        came_back("sed -n 1400,1460p src/sandboxblock_tests.rs", &text, ""),
        came_back("cat notes.log 1>&2", "", &text),
        came_back("head -80 notes.log | tail -20", &text, ""),
        came_back("grep -rn 'not permitted' src 2>&1 | head", &text, ""),
        one_string("sed -n 1,80p src/sandboxblock_tests.rs", &text),
        one_string("cat notes.log | grep -v ok", &text),
        one_string("git show HEAD:src/x.rs", &text),
        json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Read",
            "tool_input": {"file_path": output_file},
            "tool_response": {"type": "text", "file": {"filePath": output_file, "content": text}},
        }),
        json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "BashOutput",
            "tool_input": {"bash_id": "b7x2"},
            "tool_response": {"stdout": text, "stderr": "", "status": "completed"},
        }),
    ] {
        assert_eq!(
            detect_with_targets(&payload, &place()),
            Vec::new(),
            "{payload}"
        );
    }
}

/// A Codex or opencode chat's result, as the hook reads it: a `<sandbox_violations>` block in it
/// is one the command printed, since purlis's own sandbox appends none.
#[test]
fn a_violation_block_a_wrapped_harness_printed_raises_nothing() {
    let printed = format!(
        "{}\n<sandbox_violations>\ndocker(42) deny(1) network-outbound {SOCKET}\n\
         </sandbox_violations>",
        quoted()
    );
    for harness in ["codex", "opencode"] {
        let payload = as_its_harness_reports(&one_string("cat x.md", &printed), Some(harness));
        assert_eq!(
            detect_with_targets(&payload, &place()),
            Vec::new(),
            "{harness}"
        );
    }
}

/// A refusal the command met still raises its block: the failed command's own error, the
/// sandbox's own report (even beside a line that only reads), opencode's failed command, and a
/// container client whose status a pipe dropped (ADR 0093).
#[test]
fn a_refusal_the_command_met_still_raises_its_block() {
    let socket = |named: Option<&str>| {
        (
            Block {
                operation: Operation::Connect,
                kind: Kind::LocalSocket,
                ours: false,
            },
            named.map(str::to_owned),
        )
    };
    let refused = format!("dial unix {SOCKET}: connect: operation not permitted");
    // The failed command's own error.
    assert_eq!(
        detect_with_targets(
            &failed("go run ./cmd/ping", &format!("Exit code 1\n{refused}")),
            &place()
        ),
        vec![socket(Some(SOCKET))]
    );
    // The sandbox's own report, beside a command that did not fail.
    let reported = format!(
        "{refused}\n<sandbox_violations>\nping(42) deny(1) network-outbound {SOCKET}\n\
         </sandbox_violations>"
    );
    assert_eq!(
        detect_with_targets(&came_back("cat ping.sock", "", &reported), &place()),
        vec![socket(Some(SOCKET))]
    );
    // opencode's failed command: both streams, marked mixed; nothing printed names a target.
    let mixed = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "go run ./cmd/ping"},
        "tool_response": {"stderr": refused, "mixed": true},
    });
    assert_eq!(
        detect_with_targets(&mixed, &place()),
        vec![socket(Some(SOCKET))]
    );
    // A container client whose status the line dropped: its own words for its socket.
    for payload in [
        came_back("docker run --rm alpine true 2>&1 | tail", &refused, ""),
        one_string("docker run --rm alpine true 2>&1 | tail", &refused),
    ] {
        assert_eq!(
            detect_with_targets(&payload, &place()),
            vec![socket(Some(SOCKET))],
            "{payload}"
        );
    }
    // A failed lookup, and a proxy's refusal the failed command met.
    let found = detect_with_targets(
        &failed(
            "curl https://api.example.com",
            "Exit code 6\ncurl: (6) Could not resolve host: api.example.com",
        ),
        &place(),
    );
    assert_eq!(
        found,
        vec![(
            Block {
                operation: Operation::Lookup,
                kind: Kind::Host,
                ours: false,
            },
            Some("api.example.com".to_owned())
        )]
    );
}

/// A result with no failure beside it says nothing of how each program of its line ended (a
/// pipe drops a status, and Codex says nothing of it), so the line is the one signal left: a
/// line that runs a program other than a reader may have met the refusal it said. It raises
/// the block, naming no socket or port: the person names it.
#[test]
fn a_result_with_no_failure_beside_it_is_read_by_its_line() {
    let said = "Unable to connect to the server: dial tcp 127.0.0.1:50795: connect: operation \
                not permitted\n";
    assert_eq!(
        detect_with_targets(&one_string("kubectl get pods", said), &place()),
        vec![(
            Block {
                operation: Operation::Connect,
                kind: Kind::LocalService,
                ours: false,
            },
            None
        )]
    );
    assert_eq!(
        detect_with_targets(&came_back("kubectl get pods | head", said, ""), &place()),
        vec![(
            Block {
                operation: Operation::Connect,
                kind: Kind::LocalService,
                ours: false,
            },
            None
        )]
    );
    for payload in [
        one_string("cat kubectl.log", said),
        came_back("cat kubectl.log", said, ""),
    ] {
        assert_eq!(
            detect_with_targets(&payload, &place()),
            Vec::new(),
            "{payload}"
        );
    }
}

/// opencode's `PostToolUse` for a command that did not fail: one string, as its shim hands it.
fn opencode_string(command: &str, said: &str) -> serde_json::Value {
    json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "bash",
        "tool_input": {"command": command},
        "tool_response": said,
    })
}

/// A refused socket, as a Go client says it, and the block it raises where nothing names it.
fn refused_socket() -> String {
    format!("dial unix {SOCKET}: connect: operation not permitted\n")
}

fn unnamed_socket() -> Vec<(Block, Option<String>)> {
    vec![(
        Block {
            operation: Operation::Connect,
            kind: Kind::LocalSocket,
            ours: false,
        },
        None,
    )]
}

/// Every shape a result with no failure beside it comes in, for `command`: Claude Code's,
/// Codex's and opencode's. Codex and opencode have no sandbox report to fall back on.
fn every_harness(command: &str, said: &str) -> [serde_json::Value; 3] {
    [
        came_back(command, said, ""),
        one_string(command, said),
        opencode_string(command, said),
    ]
}

/// A line the guards cannot read whole may have run a program: a line too big for them, one
/// that does not tokenize, and a string a shell is handed nested past what is read each raise
/// the block, naming nothing.
#[test]
fn a_line_that_cannot_be_read_whole_still_raises_its_block() {
    let big = format!(
        "go run ./cmd/ping # {}",
        "x".repeat(crate::guardcaps::MAX_COMMAND_BYTES)
    );
    for command in [
        big.as_str(),
        "cat notes.log 'unclosed",
        "sh -c \"bash -c 'sh -c cat'\"",
    ] {
        for payload in every_harness(command, &refused_socket()) {
            assert_eq!(
                detect_with_targets(&payload, &place()),
                unnamed_socket(),
                "{payload}"
            );
        }
    }
}

/// A reader that can start a program of its own is read as one that runs a program: `find`
/// with `-exec`, `-execdir`, `-ok` or `-okdir`, `awk`'s script, and `sed` with any script but
/// lines to print, since GNU `sed` runs one with `e`. `sed -n` over a range and a plain `find`
/// still only read.
#[test]
fn a_reader_that_can_start_a_program_still_raises_its_block() {
    for command in [
        "find . -name '*.go' -exec go run {} \\;",
        "find . -execdir ./ping \\;",
        "find . -ok ./ping \\;",
        "find . -okdir ./ping \\;",
        "awk 'BEGIN { system(\"./ping\") }'",
        "gawk '{ print | \"./ping\" }' list",
        "sed 's/^/.\\/ping /e' list",
        "sed -n '1e ./ping' list",
        "sed -f run.sed list",
        "sed -i 1d list",
    ] {
        for payload in every_harness(command, &refused_socket()) {
            assert_eq!(
                detect_with_targets(&payload, &place()),
                unnamed_socket(),
                "{payload}"
            );
        }
    }
    for command in [
        "sed -n 1400,1460p src/sandboxblock_tests.rs",
        "sed -n '5p;10p;$p' notes.log",
        "sed -nE '1,$!d' notes.log",
        "find . -name '*.rs' -print",
    ] {
        for payload in every_harness(command, &refused_socket()) {
            assert_eq!(
                detect_with_targets(&payload, &place()),
                Vec::new(),
                "{payload}"
            );
        }
    }
}

/// A background command polled with the harness's own tool that ended failed is a failed
/// command: its output is its own error, and its socket is named. It is still marked read back.
#[test]
fn a_background_command_that_ended_failed_raises_its_block() {
    let said = format!(
        "docker: permission denied while trying to connect to the docker API at \
         unix://{SOCKET}: dial unix {SOCKET}: connect: operation not permitted\n"
    );
    let named = vec![(
        Block {
            operation: Operation::Connect,
            kind: Kind::LocalSocket,
            ours: false,
        },
        Some(SOCKET.to_owned()),
    )];
    for response in [
        json!({"stdout": said, "stderr": "", "status": "failed"}),
        json!({"stdout": "", "stderr": said, "status": "completed", "exitCode": 125}),
    ] {
        let polled = json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "BashOutput",
            "tool_input": {"bash_id": "b7x2"},
            "tool_response": response,
        });
        assert!(read_back(&polled));
        assert_eq!(detect_with_targets(&polled, &place()), named, "{polled}");
    }
    // One that ended well, or still runs, merely printed what it handed back.
    for response in [
        json!({"stdout": said, "stderr": "", "status": "completed", "exitCode": 0}),
        json!({"stdout": said, "stderr": "", "status": "running"}),
    ] {
        let polled = json!({
            "tool_name": "BashOutput",
            "tool_input": {"bash_id": "b7x2"},
            "tool_response": response,
        });
        assert_eq!(
            detect_with_targets(&polled, &place()),
            Vec::new(),
            "{polled}"
        );
    }
}

/// A program's own refusal of a path on standard error is read whatever the line runs: a
/// reader can be refused a path itself (`tee` writing one), and standard error is its own
/// error stream. So `cat` of a log sent to standard error is read the same way (D-1972-2).
#[test]
fn a_readers_refusal_of_a_path_on_standard_error_is_still_a_block() {
    let write = vec![(
        Block {
            operation: Operation::Write,
            kind: Kind::System,
            ours: false,
        },
        Some("/opt/tool/cache".to_owned()),
    )];
    for command in ["echo x | tee /opt/tool/cache", "cat build.log 1>&2"] {
        assert_eq!(
            detect_with_targets(
                &came_back(command, "", "tee: /opt/tool/cache: Operation not permitted"),
                &place()
            ),
            write,
            "{command}"
        );
    }
}

/// More readers that can start a program of their own (#2203): `rg --pre`, `sort
/// --compress-program`, `less` and `more` (their input preprocessor, `LESSOPEN`, is a
/// program), and `git` given configuration on its line (`-c`, `--config-env`, or
/// `GIT_CONFIG_*` and `GIT_EXTERNAL_DIFF` in its environment: `diff.external`,
/// `core.fsmonitor`), asked for an external diff, or a grep that opens its files in a pager
/// (`-O`, `--open-files-in-pager`). The same readers without those forms still only read.
#[test]
fn more_readers_that_can_start_a_program_still_raise_their_block() {
    for command in [
        "rg --pre ./ping needle",
        "rg --pre=./ping needle",
        "rg -n --pre=./ping needle src",
        "sort --compress-program=./ping big.txt",
        "sort --compress-program ./ping big.txt",
        "less notes.log",
        "LESSOPEN='|./ping %s' less notes.log",
        "more notes.log",
        "git -c diff.external=./ping diff",
        "git -c core.fsmonitor=./ping status",
        "git --config-env=core.fsmonitor=PING status",
        "GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=core.fsmonitor GIT_CONFIG_VALUE_0=./ping git status",
        "GIT_EXTERNAL_DIFF=./ping git diff",
        "git grep -O needle",
        "git grep -O./ping needle",
        "git grep --open-files-in-pager=./ping needle",
        "git diff --ext-diff",
        // Any prefix the program itself takes for the option.
        "sort --co=./ping big.txt",
        "sort --compress ./ping big.txt",
        "git grep --open-files needle",
        "git grep --op=./ping needle",
        "git diff --ext",
        "git log -p --ext-d",
    ] {
        for payload in every_harness(command, &refused_socket()) {
            assert_eq!(
                detect_with_targets(&payload, &place()),
                unnamed_socket(),
                "{payload}"
            );
        }
    }
    for command in [
        "rg -n needle src",
        "rg --pretty needle",
        "sort -u list.txt",
        "git status --short",
        "git grep -n needle",
        "git diff -O order.txt",
        "git -C repo show HEAD:src/x.rs",
        // Other options a prefix could be confused with.
        "sort --check list.txt",
        "rg --pre-glob '*.gz' needle",
        "git grep --only-matching needle",
        "git diff --exit-code",
        "git diff --no-ext-diff",
    ] {
        for payload in every_harness(command, &refused_socket()) {
            assert_eq!(
                detect_with_targets(&payload, &place()),
                Vec::new(),
                "{payload}"
            );
        }
    }
}
