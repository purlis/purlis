// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a block is read from, how it is sorted, and what is kept of it (#1338).

use super::*;
use serde_json::json;

const ROOT: &str = "/Users/dev/plane";
const CHAT: &str = "/Users/dev/plane/workspaces/alpha/repo";
const HOME: &str = "/Users/dev";

fn place() -> Place<'static> {
    Place {
        root: Path::new(ROOT),
        chat: Path::new(CHAT),
        cwd: Path::new(CHAT),
        home: Some(Path::new(HOME)),
    }
}

/// A `PostToolUseFailure` payload: the command, and the error Claude Code gives the model,
/// which holds what the command printed on both streams.
fn failed(command: &str, error: &str) -> serde_json::Value {
    json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "error": error,
    })
}

/// A `PostToolUse` payload: what the command printed, on each stream.
fn came_back(command: &str, stdout: &str, stderr: &str) -> serde_json::Value {
    json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "tool_response": {"stdout": stdout, "stderr": stderr, "interrupted": false},
    })
}

fn block(operation: Operation, kind: Kind, ours: bool) -> Block {
    Block {
        operation,
        kind,
        ours,
    }
}

/// `lines` as Claude Code appends them, after what the command printed.
fn appended(printed: &str, lines: &[&str]) -> String {
    format!(
        "{printed}\n<sandbox_violations>\n{}\n</sandbox_violations>",
        lines.join("\n")
    )
}

// ---- recorded violation lines -----------------------------------------------------------------

/// Smart close from a clone: `purlis session record` writing the workspace's sessions folder,
/// as Claude Code reported it.
const SESSION_RECORD: &str = "Exit code 1\n\
purlis: the record was accepted, but could not be saved: Operation not permitted (os error 1)\n\
<sandbox_violations>\n\
purlis(48211) deny(1) file-write-create /Users/dev/plane/workspaces/alpha/sessions/2026-10-06-close.md\n\
purlis(48211) deny(1) file-write-data /Users/dev/plane/workspaces/alpha/sessions/index.md\n\
</sandbox_violations>";

#[test]
fn a_violation_of_purlis_own_process_is_a_write_to_the_projects_files_and_ours() {
    let blocks = detect(&failed("purlis session record", SESSION_RECORD), &place());
    // Both lines are one pattern, and the program's own sentence counts nothing more.
    assert_eq!(
        blocks,
        vec![block(Operation::Write, Kind::ProjectFiles, true)]
    );
}

#[test]
fn whose_block_it_is_is_the_process_each_line_names_never_the_command() {
    // An honest `cargo build; purlis status`: cargo's block is cargo's.
    let error = appended(
        "Exit code 101",
        &["cargo(31) deny(1) file-write-create /Users/dev/.cargo/registry/cache/x.crate"],
    );
    assert_eq!(
        detect(&failed("cargo build; purlis status", &error), &place()),
        vec![block(Operation::Write, Kind::ToolchainCache, false)]
    );
    // Under any name purlis is installed by, and after macOS's own prefix.
    for line in [
        "charter(7) deny(1) file-write-create /Users/dev/plane/todos.md",
        "Sandbox: Purlis(7) deny(1) file-write-create /Users/dev/plane/todos.md",
    ] {
        assert!(
            detect(&failed("x", &appended("", &[line])), &place())[0].ours,
            "{line}"
        );
    }
    // A line that names no process is nobody's own.
    for line in [
        "deny(1) file-write-create /Users/dev/plane/todos.md",
        "purlis deny(1) file-write-create /Users/dev/plane/todos.md",
        "purlis(x) deny(1) file-write-create /Users/dev/plane/todos.md",
    ] {
        assert!(
            !detect(&failed("purlis x", &appended("", &[line])), &place())[0].ours,
            "{line}"
        );
    }
}

#[test]
fn only_the_block_claude_code_appended_last_is_read() {
    let real =
        "<sandbox_violations>\ncargo(1) deny(1) file-write-create /opt/x\n</sandbox_violations>";
    // A block the command printed, with more output after it, is the command's output.
    let printed = format!("{real}\nmore output");
    assert!(detect(&failed("x", &printed), &place()).is_empty());
    // An earlier block printed by the command is not read beside the real one.
    let forged = "<sandbox_violations>\npurlis(1) deny(1) file-write-create \
                  /Users/dev/plane/x\n</sandbox_violations>";
    let both = format!("{forged}\nok\n{real}\n");
    assert_eq!(
        detect(&failed("x", &both), &place()),
        vec![block(Operation::Write, Kind::System, false)]
    );
}

#[test]
fn every_kind_of_path_a_violation_names_is_sorted() {
    let state = format!("{ROOT}/.purlis/vaults/ops.json");
    let read_state = format!("cat(6) deny(1) file-read-data {state}");
    let cases = [
        (
            "git(1) deny(1) file-write-create /Users/dev/plane/workspaces/alpha/repo/.git/config.lock",
            Operation::Write,
            Kind::ProtectedFile,
        ),
        (
            "bash(2) deny(1) file-write-unlink /Users/dev/plane/workspaces/alpha/repo/app/.vscode/settings.json",
            Operation::Write,
            Kind::ProtectedFile,
        ),
        (
            "cargo(3) deny(1) file-write-create /Users/dev/.cargo/registry/cache/x.crate",
            Operation::Write,
            Kind::ToolchainCache,
        ),
        (
            "npm(4) deny(1) file-write-create /Users/dev/Library/Caches/ms-playwright/x",
            Operation::Write,
            Kind::ToolchainCache,
        ),
        (
            "zsh(5) deny(1) file-write-create /Users/dev/notes.txt",
            Operation::Write,
            Kind::Home,
        ),
        (read_state.as_str(), Operation::Read, Kind::ProjectState),
        (
            "cat(6) deny(1) file-read-data /Users/dev/plane/workspaces/beta/workspace.md",
            Operation::Read,
            Kind::ProjectFiles,
        ),
        (
            "x(6) deny(1) file-write-create /Users/dev/plane/workspaces/alpha/repo/src/a.rs",
            Operation::Write,
            Kind::ChatFolder,
        ),
        (
            "touch(7) deny(1) file-write-create /opt/homebrew/thing",
            Operation::Write,
            Kind::System,
        ),
        (
            "touch(7) deny(1) file-write-create /private/var/folders/ab/T/x",
            Operation::Write,
            Kind::Temp,
        ),
        (
            "gh(8) deny(1) mach-lookup com.apple.trustd.agent",
            Operation::Lookup,
            Kind::CertificateCheck,
        ),
        (
            "ps(9) deny(1) mach-lookup com.apple.system.opendirectoryd.libinfo",
            Operation::Lookup,
            Kind::SystemService,
        ),
        (
            "curl(10) deny(1) network-outbound 93.184.216.34:443",
            Operation::Connect,
            Kind::Host,
        ),
        (
            "nc(11) deny(1) network-outbound /private/tmp/purlis-501/hooks.sock",
            Operation::Connect,
            Kind::LocalSocket,
        ),
        (
            "sh(12) deny(1) process-exec /usr/local/bin/thing",
            Operation::Run,
            Kind::System,
        ),
        ("sh(13) deny(1) signal", Operation::Other, Kind::System),
    ];
    for (line, operation, kind) in cases {
        assert_eq!(
            detect(&failed("x", &appended("Exit code 1", &[line])), &place()),
            vec![block(operation, kind, false)],
            "{line}"
        );
    }
}

#[test]
fn the_chats_folder_is_where_it_was_started_not_where_the_command_ran() {
    let moved = Place {
        cwd: Path::new("/Users/dev/plane/workspaces/beta"),
        ..place()
    };
    let error = appended(
        "",
        &["x(1) deny(1) file-write-create /Users/dev/plane/workspaces/beta/a.md"],
    );
    assert_eq!(
        detect(&failed("cd ../../beta && touch a.md", &error), &moved),
        vec![block(Operation::Write, Kind::ProjectFiles, false)]
    );
}

/// A manifest is protected only where the sandbox holds one (#1336): at the project root and in
/// each folder from the chat's up to it. Below the chat's folder it is the chat's own file.
#[test]
fn a_manifest_is_a_protected_file_only_where_the_sandbox_holds_it() {
    for (path, kind) in [
        (format!("{ROOT}/purlis.toml"), Kind::ProtectedFile),
        (format!("{ROOT}/charter.local.toml"), Kind::ProtectedFile),
        (
            format!("{ROOT}/workspaces/charter.toml"),
            Kind::ProtectedFile,
        ),
        (format!("{CHAT}/purlis.local.toml"), Kind::ProtectedFile),
        (format!("{CHAT}/sub/purlis.toml"), Kind::ChatFolder),
        (
            format!("{ROOT}/workspaces/beta/charter.toml"),
            Kind::ProjectFiles,
        ),
    ] {
        assert_eq!(kind_of(Path::new(&path), &place()), kind, "{path}");
    }
}

// ---- a program's own words, on its standard error ---------------------------------------------

/// What a probe of each program printed when the sandbox refused it, and how it sorts.
#[test]
fn a_programs_operation_not_permitted_names_its_path_and_is_sorted() {
    let cases = [
        (
            "touch: /opt/x: Operation not permitted",
            Operation::Write,
            Kind::System,
        ),
        (
            "mkdir: cannot create directory '/Users/dev/.npm/_cacache': Operation not permitted",
            Operation::Write,
            Kind::ToolchainCache,
        ),
        (
            "mkdir: .claude/skills: Operation not permitted",
            Operation::Write,
            Kind::ProtectedFile,
        ),
        (
            "error: could not lock config file .git/config: Operation not permitted",
            Operation::Write,
            Kind::ProtectedFile,
        ),
        (
            "Error: EPERM: operation not permitted, mkdir '/Users/dev/plane/personas/ops/memory'",
            Operation::Write,
            Kind::ProjectFiles,
        ),
        (
            "PermissionError: [Errno 1] Operation not permitted: '/Users/dev/plane/.purlis/app/x'",
            Operation::File,
            Kind::ProjectState,
        ),
        (
            "cat: ../../../todos.md: Operation not permitted",
            Operation::File,
            Kind::ProjectFiles,
        ),
        // Go, and so gh: lower case, no program in front.
        (
            "open /Users/dev/.config/gh/hosts.yml: operation not permitted",
            Operation::File,
            Kind::Home,
        ),
        (
            "failed to write config: mkdir /Users/dev/.config/gh: operation not permitted",
            Operation::Write,
            Kind::Home,
        ),
        // git's relative work tree, read from where the command ran.
        (
            "fatal: could not create work tree dir '../../beta/svc': Operation not permitted",
            Operation::Write,
            Kind::ProjectFiles,
        ),
        // git's copy names its source first; the destination is what was refused.
        (
            "fatal: cannot copy '/usr/share/git-core/templates/hooks/pre-push.sample' to \
             '/Users/dev/plane/workspaces/alpha/repo/.git/hooks/pre-push.sample': Operation not \
             permitted",
            Operation::Write,
            Kind::ProtectedFile,
        ),
        // Rust's io error with its path in the line.
        (
            "error: failed to open `/Users/dev/.cargo/.package-cache`: Operation not permitted \
             (os error 1)",
            Operation::File,
            Kind::ToolchainCache,
        ),
    ];
    for (line, operation, kind) in cases {
        assert_eq!(
            detect(&came_back("x", "", line), &place()),
            vec![block(operation, kind, false)],
            "{line}"
        );
    }
}

#[test]
fn cargos_caused_by_names_its_path_on_a_line_before() {
    let stderr = "error: failed to download `serde v1.0.0`\n\n\
                  Caused by:\n  failed to create directory `/Users/dev/.cargo/registry/cache/x`\n\n\
                  Caused by:\n  Operation not permitted (os error 1)\n";
    assert_eq!(
        detect(&came_back("cargo build", "", stderr), &place()),
        vec![block(Operation::Write, Kind::ToolchainCache, false)]
    );
}

#[test]
fn a_programs_refusal_of_what_the_chat_may_write_is_not_the_sandboxs() {
    // Its own folder and its temp folder (Claude Code's, under /tmp) are writable: a refusal
    // there is something else (a file's own flags, macOS's privacy controls), never a sandbox
    // block. macOS's per-user folders under /var/folders are not, and a write refused there is
    // the next test's.
    for line in [
        "touch: /Users/dev/plane/workspaces/alpha/repo/src/a.rs: Operation not permitted",
        "rm: /private/tmp/claude-501/x: Operation not permitted",
        "touch: src/a.rs: Operation not permitted",
    ] {
        assert!(
            detect(&came_back("x", "", line), &place()).is_empty(),
            "{line}"
        );
    }
}

const PER_USER_T: &str = "/var/folders/wl/hs3pdt2j0t9gyc_l73py38l80000gp/T";
const PER_USER_C: &str = "/var/folders/wl/hs3pdt2j0t9gyc_l73py38l80000gp/C";

/// What macOS tools that ignore `TMPDIR` printed in a sandboxed chat (#1120, measured on macOS
/// 26.2): they ask the system for the per-user temporary or cache folder instead, which no
/// sandboxed chat may write, so each is a block the notice names with its way round.
#[test]
fn a_tool_that_ignores_tmpdir_is_told_apart_from_the_chats_own_temp() {
    let (t, c) = (PER_USER_T, PER_USER_C);
    for (line, kind) in [
        (
            format!("mktemp: mkstemp failed on {t}/tmp.mWjq8inJrH: Operation not permitted"),
            Kind::SystemTemp,
        ),
        (
            format!("mktemp: mkdtemp failed on {t}/tmp.ZqSxZqUwQX: Operation not permitted"),
            Kind::SystemTemp,
        ),
        (
            format!("rm: /private{t}/x: Operation not permitted"),
            Kind::SystemTemp,
        ),
    ] {
        assert_eq!(
            detect(&came_back("x", "", &line), &place()),
            vec![block(Operation::Write, kind, false)],
            "{line}"
        );
    }
    for (line, kind) in [
        (
            format!("mktemp(1) deny(1) file-write-create /private{t}/tmp.G9c8tiJuCM"),
            Kind::SystemTemp,
        ),
        (
            format!("xcrun(2) deny(1) file-write-create /private{t}/xcrun_db-5UlOOWst"),
            Kind::SystemTemp,
        ),
        (
            format!("swift(3) deny(1) file-write-create /private{t}/p1120.txt"),
            Kind::SystemTemp,
        ),
        (
            format!(
                "swift-frontend(4) deny(1) file-write-create \
                 /private{c}/clang/ModuleCache/1XGORMFR2JUL/SwiftShims-6PVXR3VD9JVP.pcm"
            ),
            Kind::SystemCache,
        ),
    ] {
        assert_eq!(
            detect(&failed("x", &appended("Exit code 1", &[&line])), &place()),
            vec![block(Operation::Write, kind, false)],
            "{line}"
        );
    }
}

/// A read of the per-user folders that macOS's privacy controls refuse, not the sandbox: `du`,
/// `find` and `ls` over them print the same words, and are no block.
#[test]
fn a_programs_refused_read_of_the_per_user_folders_is_not_a_block() {
    for line in [
        format!("du: {PER_USER_C}/com.apple.WebKit.WebContent.Sandbox: Operation not permitted"),
        format!("find: {PER_USER_C}/com.apple.x: Operation not permitted"),
        format!("ls: /private{PER_USER_T}/com.apple.y: Operation not permitted"),
    ] {
        assert!(
            detect(&came_back("x", "", &line), &place()).is_empty(),
            "{line}"
        );
    }
}

/// The per-user folders' notice gives the chat's own `$TMPDIR` as the way round, naming no one
/// tool's option, which fits none of `0` and `X` (#1416); the cache folder's says Swift and clang
/// builds cannot write it yet, and offers no variable to set.
#[test]
fn the_per_user_folders_notices_name_each_folder_and_its_way_round() {
    let temp = block(Operation::Write, Kind::SystemTemp, false).said();
    assert!(temp.contains("temporary folder"), "{temp}");
    assert!(temp.contains("under \"$TMPDIR\" instead"), "{temp}");
    assert!(!temp.contains("mktemp"), "{temp}");
    let cache = block(Operation::Write, Kind::SystemCache, false).said();
    assert!(cache.contains("cache folder"), "{cache}");
    assert!(cache.contains("Swift and clang builds"), "{cache}");
    assert!(
        !cache.contains("TMPDIR") && !cache.contains("temporary"),
        "{cache}"
    );
}

/// #1416: `system-temp` also holds the per-user folders `0` and `X`, which are no temporary
/// folder, so its sentence names the per-user folders and holds the temporary one up as one of
/// them; and the cache folder's sentence names no harness, so it reads true in any chat.
#[test]
fn the_per_user_folders_sentences_fit_every_folder_and_harness_they_are_said_of() {
    let temp = block(Operation::Write, Kind::SystemTemp, false).said();
    assert!(temp.contains("one of macOS's per-user folders"), "{temp}");
    assert!(!temp.contains("per-user temporary folder"), "{temp}");
    let cache = block(Operation::Write, Kind::SystemCache, false).said();
    for harness in ["Claude Code", "opencode", "Codex"] {
        assert!(!cache.contains(harness), "{cache}");
    }
    assert!(cache.contains("harness runs its own sandbox"), "{cache}");
}

#[test]
fn a_refused_connection_or_certificate_check_is_read_from_standard_error() {
    let cases = [
        (
            "curl: (56) CONNECT tunnel failed, response 403",
            Operation::Connect,
            Kind::Host,
        ),
        (
            "purlis's sandbox does not allow example.org:443: no egress preset of this project \
             lists it",
            Operation::Connect,
            Kind::Host,
        ),
        (
            "Connection blocked by network allowlist",
            Operation::Connect,
            Kind::Host,
        ),
        (
            "Get \"https://api.github.com/\": tls: failed to verify certificate: x509: OSStatus \
             -26276",
            Operation::Lookup,
            Kind::CertificateCheck,
        ),
    ];
    for (line, operation, kind) in cases {
        assert_eq!(
            detect(&came_back("gh api x", "", line), &place()),
            vec![block(operation, kind, false)],
            "{line}"
        );
    }
}

#[test]
fn a_failed_commands_error_is_read_for_its_violation_block_and_network_refusals_alone() {
    // `error` holds standard output too: a program's refusal of a path in it is not read. A
    // refusal of the network is, as standard output's is (#1663), naming no host to allow.
    assert!(
        detect(
            &failed("x", "Exit code 1\ntouch: /opt/x: Operation not permitted"),
            &place()
        )
        .is_empty()
    );
    assert_eq!(
        detect_with_targets(
            &failed(
                "x",
                "Exit code 6\ncurl: (56) CONNECT tunnel failed, response 403"
            ),
            &place()
        ),
        vec![(block(Operation::Connect, Kind::Host, false), None)]
    );
}

/// A Go program refused the certificate check exits non-zero, so its words reach the hook only
/// in the failure's `error` (or on standard output, run with `2>&1`): `glab api` failed so in a
/// sandboxed chat and no Notice came up, and the chat sent the person looking for one.
#[test]
fn a_go_programs_refused_certificate_check_is_read_from_a_failed_commands_error() {
    // Named as the one service a grant names (ADR 0093).
    let certificate = vec![(
        block(Operation::Lookup, Kind::CertificateCheck, false),
        Some(crate::sandbox::services::TRUSTD.to_owned()),
    )];
    for error in [
        "Exit code 1\nPOST https://gitlab.com/api/v4/projects/1/issues: tls: failed to verify \
         certificate: x509: OSStatus -26276",
        "Exit code 1\nPost \"https://api.github.com/graphql\": tls: failed to verify \
         certificate: x509: OSStatus -26276.",
    ] {
        assert_eq!(
            detect_with_targets(&failed("glab api -X POST x", error), &place()),
            certificate,
            "{error}"
        );
    }
    // A command that only read the words raises nothing (#1808's review, MF-5; #1972).
    assert_eq!(
        detect_with_targets(
            &came_back(
                "cat glab.log",
                "tls: failed to verify certificate: x509: OSStatus -26276\n",
                ""
            ),
            &place()
        ),
        Vec::new()
    );
}

#[test]
fn a_line_that_only_quotes_the_certificate_checks_words_is_not_a_block() {
    // purlis's own source, or a log about the refusal, read back on standard output.
    for line in [
        "src/sandboxblock.rs:940:    if line.contains(\"x509\") && line.contains(\"-26276\") {",
        "x509: OSStatus -26276 means the system's certificate check was refused",
    ] {
        assert!(
            detect(&came_back("grep -rn 26276 src", line, ""), &place()).is_empty(),
            "{line}"
        );
        assert!(
            detect(&failed("grep -rn 26276 src", line), &place()).is_empty(),
            "{line}"
        );
    }
}

#[test]
fn what_a_command_printed_on_its_standard_output_is_never_a_block_of_a_path() {
    // A chat reading purlis's own sources, or a log, prints these words without being refused.
    let stdout = format!("{SESSION_RECORD}\ntouch: /opt/x: Operation not permitted\n");
    assert!(detect(&came_back("cat log.txt", &stdout, ""), &place()).is_empty());
}

#[test]
fn a_line_that_only_quotes_the_words_is_not_a_block() {
    for line in [
        "src/browser.rs:492:            \"mkdir: .claude/skills: Operation not permitted\\n\",",
        "Operation not permitted (os error 1)",
        "ps: Operation not permitted",
        "warning: the words `x: y: Operation not permitted` mean z",
    ] {
        assert!(
            detect(&came_back("x", "", line), &place()).is_empty(),
            "{line}"
        );
    }
}

#[test]
fn a_command_that_failed_on_many_files_is_a_few_blocks_none_twice() {
    let mut lines = Vec::new();
    for n in 0..500 {
        lines.push(format!(
            "cp({n}) deny(1) file-write-create /Users/dev/plane/workspaces/beta/{n}"
        ));
    }
    for n in 0..20 {
        lines.push(format!("x(1) deny(1) mach-lookup com.example.{n}"));
    }
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    let blocks = detect(&failed("cp -r a b", &appended("", &lines)), &place());
    assert_eq!(
        blocks,
        vec![
            block(Operation::Write, Kind::ProjectFiles, false),
            block(Operation::Lookup, Kind::SystemService, false),
        ]
    );
}

#[test]
fn a_tool_result_that_is_one_string_is_read_for_its_violation_block_alone() {
    let payload = |response: &str| json!({"tool_name": "Bash", "tool_input": {"command": "x"}, "tool_response": response});
    assert!(detect(&payload("touch: /opt/x: Operation not permitted"), &place()).is_empty());
    assert_eq!(
        detect(
            &payload(&appended(
                "",
                &["touch(1) deny(1) file-write-create /opt/x"]
            )),
            &place()
        ),
        vec![block(Operation::Write, Kind::System, false)]
    );
}

// ---- how often one is taken -------------------------------------------------------------------

#[test]
fn a_chat_is_heard_once_a_minute_per_block_and_a_handful_a_minute_in_all() {
    let mut throttle = Throttle::default();
    let now = std::time::Instant::now();
    let write = block(Operation::Write, Kind::ProjectFiles, true);
    assert!(throttle.lets(3, &write, now));
    assert!(
        !throttle.lets(3, &write, now),
        "the same block again at once"
    );
    assert!(throttle.lets(4, &write, now), "another chat's is its own");
    let let_through = Kind::ALL
        .into_iter()
        .filter(|kind| *kind != Kind::ProjectFiles)
        .filter(|kind| throttle.lets(3, &block(Operation::Write, *kind, false), now))
        .count();
    assert_eq!(
        let_through,
        Throttle::PER_CHAT - 1,
        "the chat's share of the minute"
    );
    let later = now + THROTTLE_WINDOW;
    assert!(
        throttle.lets(3, &write, later),
        "a minute on, it is heard again"
    );
}

/// A host refused a moment after another is its own block, with its own Notice to allow it:
/// the same block is the same block on the same target, whichever road told it.
#[test]
fn another_host_refused_within_the_minute_is_heard_and_the_same_one_is_not() {
    let mut throttle = Throttle::default();
    let now = std::time::Instant::now();
    let host = block(Operation::Connect, Kind::Host, false);
    assert!(throttle.lets_on(3, &host, Some("api.example.com:443"), now));
    assert!(
        throttle.lets_on(3, &host, Some("cluster.example-k8s.com:6443"), now),
        "another host is its own block"
    );
    assert!(
        !throttle.lets_on(3, &host, Some("API.example.com:443"), now),
        "the same host again at once, in any case"
    );
    assert!(
        throttle.lets_on(3, &host, None, now),
        "a host the report did not name is its own block too"
    );
    assert!(!throttle.lets(3, &host, now), "and is that one again");
    // Still at most a handful a minute, whatever hosts a chat names.
    let let_through = (0..100)
        .filter(|n| throttle.lets_on(3, &host, Some(&format!("h{n}.example:443")), now))
        .count();
    assert_eq!(let_through, Throttle::PER_CHAT - 3);
}

/// #1681: the repeats the throttle holds back are counted, not dropped: once a block's minute
/// is over, how many times it came again in it is told once, for the record's own line.
#[test]
fn the_repeats_held_back_in_a_minute_are_told_once_the_minute_is_over() {
    let mut throttle = Throttle::default();
    let now = std::time::Instant::now();
    let host = block(Operation::Connect, Kind::Host, false);
    let write = block(Operation::Write, Kind::ProjectFiles, false);
    assert!(throttle.lets_on(3, &host, Some("api.example.com:443"), now));
    let soon = now + std::time::Duration::from_secs(5);
    assert!(!throttle.lets_on(3, &host, Some("API.example.com:443"), soon));
    let later = now + std::time::Duration::from_secs(20);
    assert!(!throttle.lets_on(3, &host, Some("api.example.com:443"), later));
    assert!(throttle.lets(4, &write, now));
    assert!(
        throttle.repeats_over(now + THROTTLE_WINDOW / 2).is_empty(),
        "not while its minute runs"
    );

    let over = now + THROTTLE_WINDOW;
    assert_eq!(
        throttle.repeats_over(over),
        [Repeated {
            chat: 3,
            block: host,
            target: Some("api.example.com:443".to_owned()),
            times: 2,
            last: later,
        }],
        "a block that never came again is told nothing"
    );
    assert!(throttle.repeats_over(over).is_empty(), "told once");
}

/// A block heard anew a minute on is let through, and the repeats of the minute before it are
/// still told, not lost to the new minute.
#[test]
fn a_block_heard_anew_still_tells_the_repeats_of_its_minute_before() {
    let mut throttle = Throttle::default();
    let now = std::time::Instant::now();
    let write = block(Operation::Write, Kind::ProjectFiles, false);
    assert!(throttle.lets(3, &write, now));
    assert!(!throttle.lets(3, &write, now));
    let next = now + THROTTLE_WINDOW;
    assert!(throttle.lets(3, &write, next));
    assert_eq!(
        throttle
            .repeats_over(next)
            .iter()
            .map(|one| (one.chat, one.times))
            .collect::<Vec<_>>(),
        [(3, 1)]
    );
}

/// A chat that goes quiet or ends has its held-back repeats told at once, not at the next
/// block the project hears (qw146 follow-up): its throttle keeps holding the flood back, and a
/// repeat after the flush is counted afresh, so nothing is told twice.
#[test]
fn a_quiet_or_ended_chats_repeats_are_told_at_once_and_counted_afresh() {
    let mut throttle = Throttle::default();
    let now = std::time::Instant::now();
    let write = block(Operation::Write, Kind::ProjectFiles, false);
    assert!(throttle.lets(3, &write, now));
    assert!(!throttle.lets(3, &write, now));
    assert!(!throttle.lets(3, &write, now));
    assert!(throttle.lets(4, &write, now));
    assert!(!throttle.lets(4, &write, now));
    let told = |over: Vec<Repeated>| -> Vec<(u32, u64)> {
        over.iter().map(|one| (one.chat, one.times)).collect()
    };
    assert_eq!(told(throttle.repeats_of(3)), [(3, 2)]);
    assert!(throttle.repeats_of(3).is_empty(), "told once");
    assert!(
        !throttle.lets(3, &write, now),
        "still held back in its minute"
    );
    assert_eq!(told(throttle.repeats_of(3)), [(3, 1)], "counted afresh");
    assert_eq!(told(throttle.every_repeat()), [(4, 1)], "the app quits");
    assert!(throttle.every_repeat().is_empty());
    assert!(throttle.repeats_over(now + THROTTLE_WINDOW).is_empty());
}

/// A repeat whose minute ended while another chat's block was heard, not told yet, is told with
/// its chat's flush.
#[test]
fn a_repeat_whose_minute_ended_is_told_with_its_chats_flush() {
    let mut throttle = Throttle::default();
    let now = std::time::Instant::now();
    let write = block(Operation::Write, Kind::ProjectFiles, false);
    assert!(throttle.lets(3, &write, now));
    assert!(!throttle.lets(3, &write, now));
    let next = now + THROTTLE_WINDOW;
    assert!(throttle.lets(3, &write, next));
    assert_eq!(
        throttle
            .repeats_of(3)
            .iter()
            .map(|one| (one.chat, one.times))
            .collect::<Vec<_>>(),
        [(3, 1)]
    );
}

#[test]
fn a_block_says_its_operation_and_kind_in_one_phrase() {
    assert_eq!(
        block(Operation::Write, Kind::ProjectFiles, true).said(),
        "a write to the project's own files"
    );
    assert_eq!(
        block(Operation::Connect, Kind::Host, false).said(),
        "a connection to an internet host this project does not allow"
    );
}

#[test]
fn every_word_reads_back_as_what_it_names() {
    for operation in Operation::ALL {
        assert_eq!(Operation::of_word(operation.word()), Some(operation));
        assert_eq!(
            serde_json::to_value(operation).unwrap(),
            json!(operation.word())
        );
    }
    for kind in Kind::ALL {
        assert_eq!(Kind::of_word(kind.word()), Some(kind));
        assert_eq!(serde_json::to_value(kind).unwrap(), json!(kind.word()));
    }
    assert_eq!(Operation::of_word("/etc/passwd"), None);
    assert_eq!(Kind::of_word("../x"), None);
}

// ---- what a grant could name (#1342) --------------------------------------------------------

#[test]
fn a_refused_write_or_host_carries_what_a_grant_would_name_and_nothing_else_does() {
    let targets = |payload: serde_json::Value| -> Vec<Option<String>> {
        detect_with_targets(&payload, &place())
            .into_iter()
            .map(|(_, target)| target)
            .collect()
    };
    // A write outside the project, as Seatbelt names it, and a relative one read from the cwd.
    assert_eq!(
        targets(failed(
            "x",
            &appended(
                "",
                &[
                    "cargo(1) deny(1) file-write-create /Users/dev/.cargo/registry/x.lock",
                    "touch(2) deny(1) file-write-create /opt/tool/cache",
                ]
            )
        )),
        [
            Some("/Users/dev/.cargo/registry/x.lock".to_owned()),
            Some("/opt/tool/cache".to_owned())
        ]
    );
    assert_eq!(
        targets(came_back(
            "x",
            "",
            "touch: ../../beta/x: Operation not permitted"
        )),
        [Some("/Users/dev/plane/workspaces/beta/x".to_owned())]
    );
    // A host, where the report names one, and none where it does not.
    assert_eq!(
        targets(came_back(
            "x",
            "",
            "purlis's sandbox does not allow example.org:443: no egress preset or host of this \
             project lists it"
        )),
        [Some("example.org:443".to_owned())]
    );
    assert_eq!(
        targets(failed(
            "x",
            &appended("", &["curl(10) deny(1) network-outbound 93.184.216.34:443"])
        )),
        [Some("93.184.216.34:443".to_owned())]
    );
    assert_eq!(
        targets(came_back(
            "x",
            "",
            "Connection blocked by network allowlist"
        )),
        [None]
    );
    // A socket is named whole, as a grant names it (ADR 0093).
    assert_eq!(
        targets(failed(
            "x",
            &appended(
                "",
                &["nc(5) deny(1) network-outbound /private/tmp/purlis-501/hooks.sock"]
            )
        )),
        [Some("/private/tmp/purlis-501/hooks.sock".to_owned())]
    );
    // Never for purlis's own state, a protected file, a read, or purlis's own process.
    for line in [
        "git(3) deny(1) file-write-create /Users/dev/plane/.purlis/app/x",
        "git(3) deny(1) file-write-create /Users/dev/plane/workspaces/alpha/repo/.git/config",
        "cat(4) deny(1) file-read-data /Users/dev/.config/purlis/vaults/x",
        "purlis(6) deny(1) file-write-create /Users/dev/plane/workspaces/beta/sessions/x.md",
    ] {
        assert_eq!(
            targets(failed("x", &appended("", &[line]))),
            [None],
            "{line}"
        );
    }
    // What `detect` keeps is the block alone, as it was.
    assert_eq!(
        detect(
            &failed(
                "x",
                &appended("", &["touch(2) deny(1) file-write-create /opt/x"])
            ),
            &place()
        ),
        [block(Operation::Write, Kind::System, false)]
    );
}

#[test]
fn purlis_s_own_refusal_wording_is_purlis_s_own_block_with_nothing_to_grant() {
    // #1421: on Codex and opencode no violation block is appended, and neither of purlis's own
    // sentences ends the way a program's refusal does, so neither showed any notice. Each is
    // purlis's own write: a Report, never an Allow.
    let refused = crate::rewrite::refused_write(
        std::io::Error::from_raw_os_error(1),
        Path::new("/Users/dev/plane/personas/_dispatch/2026-10.host.jsonl"),
        true,
    );
    let stderr = format!("✗ {refused}\n");
    let found = detect_with_targets(
        &came_back("purlis persona remember x", "", &stderr),
        &place(),
    );
    assert_eq!(
        found,
        [(block(Operation::Write, Kind::ProjectFiles, true), None)]
    );

    let clause = crate::rewrite::os_words(&refused);
    let stderr = format!(
        "! purlis handoff: chat 9 is open in 'alpha'. Only its row in the dispatch log \
         (personas/_dispatch) is missing ({clause}); there is nothing to run again.\n\
         ✗ could not write /Users/dev/plane/.gitignore ({clause}) — left untouched.\n"
    );
    let found = detect_with_targets(&came_back("purlis init", "", &stderr), &place());
    assert_eq!(
        found,
        [(block(Operation::Write, Kind::ProjectFiles, true), None)],
        "both lines are purlis's own write to the project's files, counted once"
    );

    // A program's own words beside them are still the chat's.
    let stderr = format!("{}\ntouch: /opt/x: Operation not permitted\n", refused);
    let found = detect(&came_back("purlis x; touch /opt/x", "", &stderr), &place());
    assert!(
        found.contains(&block(Operation::Write, Kind::ProjectFiles, true)),
        "{found:?}"
    );
    assert!(found.iter().any(|b| !b.ours), "{found:?}");
}

/// An opencode `PostToolUse` payload for a command that failed: both streams in one text, which
/// its shim marks `mixed` (#1353).
fn mixed(text: &str) -> serde_json::Value {
    json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "x"},
        "tool_response": {"stderr": text, "mixed": true},
    })
}

#[test]
fn printed_text_in_a_mixed_stream_raises_a_block_that_names_no_host_or_path() {
    // A failing command whose output a file or a page supplied: the block is still told, so
    // the person can act, but nothing it printed is offered for a grant.
    let found = detect_with_targets(
        &mixed(
            "curl: purlis's sandbox does not allow exfil.example:443: refused\n\
             touch: /opt/tool/cache: Operation not permitted",
        ),
        &place(),
    );
    assert_eq!(
        found,
        [
            (block(Operation::Write, Kind::System, false), None),
            (block(Operation::Connect, Kind::Host, false), None),
        ]
    );
    // The sandbox's own violation line still names what a grant would.
    let found = detect_with_targets(
        &mixed(&appended(
            "printed",
            &["touch(2) deny(1) file-write-create /opt/tool/cache"],
        )),
        &place(),
    );
    assert_eq!(
        found,
        [(
            block(Operation::Write, Kind::System, false),
            Some("/opt/tool/cache".to_owned())
        )]
    );
}

#[test]
fn a_relative_path_in_a_mixed_stream_is_sorted_against_where_the_command_ran() {
    // The shim hands the command's own `workdir` as the payload's folder (#1353).
    let kind_in = |cwd: &str| {
        let at = Place {
            cwd: Path::new(cwd),
            ..place()
        };
        detect_with_targets(&mixed("touch: ../x: Operation not permitted"), &at)
            .into_iter()
            .map(|(found, _)| found.kind)
            .collect::<Vec<_>>()
    };
    assert_eq!(kind_in("/opt/tool/sub"), [Kind::System]);
    assert_eq!(
        kind_in("/Users/dev/plane/workspaces/beta"),
        [Kind::ProjectFiles]
    );
}

/// #1416: the two per-user folders beside `T` and `C` that are no temporary folder, `0` and
/// `X`, sort as per-user folders, not as the chat's own temp, so a write refused there is not
/// dropped from standard error; and none of them is offered as a grant's target.
#[test]
fn the_per_user_folders_0_and_x_sort_as_per_user_folders_and_offer_no_grant() {
    let at = "/var/folders/wl/hs3pdt2j0t9gyc_l73py38l80000gp";
    for which in ["0", "X"] {
        for spelled in [
            format!("{at}/{which}/com.apple.x/y"),
            format!("/private{at}/{which}/com.apple.x/y"),
        ] {
            assert_eq!(
                kind_of(Path::new(&spelled), &place()),
                Kind::SystemTemp,
                "{spelled}"
            );
            let line = format!("touch: {spelled}: Operation not permitted");
            assert_eq!(
                detect_with_targets(&came_back("x", "", &line), &place()),
                vec![(block(Operation::Write, Kind::SystemTemp, false), None)],
                "{line}"
            );
        }
    }
    // A read there is macOS's privacy controls, as for `T` and `C`.
    let read = format!("du: {at}/0/com.apple.x: Operation not permitted");
    assert!(detect(&came_back("du", "", &read), &place()).is_empty());
}

/// #1416: the data volume's own spelling of a path, `/System/Volumes/Data/…`, sorts as the path
/// it names, and so is never offered as a grant's target where that path would not be.
#[test]
fn a_path_spelled_through_the_data_volume_sorts_as_the_path_it_names() {
    let data = "/System/Volumes/Data";
    for (path, kind) in [
        (format!("{data}{PER_USER_T}/tmp.x"), Kind::SystemTemp),
        (
            format!("{data}/private{PER_USER_T}/tmp.x"),
            Kind::SystemTemp,
        ),
        (
            format!("{data}/private{PER_USER_C}/clang/ModuleCache/a.pcm"),
            Kind::SystemCache,
        ),
        (format!("{data}{ROOT}/.purlis/app/x"), Kind::ProjectState),
        (format!("{data}{CHAT}/src/a.rs"), Kind::ChatFolder),
        (
            format!("{data}{HOME}/.cargo/registry/x"),
            Kind::ToolchainCache,
        ),
        (format!("{data}/private/tmp/x"), Kind::Temp),
    ] {
        assert_eq!(kind_of(Path::new(&path), &place()), kind, "{path}");
    }
    let line =
        format!("xcrun(2) deny(1) file-write-create {data}/private{PER_USER_T}/xcrun_db-5UlOOWst");
    assert_eq!(
        detect_with_targets(&failed("x", &appended("Exit code 1", &[&line])), &place()),
        vec![(block(Operation::Write, Kind::SystemTemp, false), None)]
    );
}

/// #1416: swiftc's and xcrun's own words for a write the sandbox refused them, which name the
/// per-user folder in quotes and the system's words after it.
#[test]
fn swiftc_and_xcrun_refusals_are_read_from_standard_error() {
    let (t, c) = (PER_USER_T, PER_USER_C);
    for (line, kind) in [
        (
            format!(
                "<unknown>:0: error: unable to open output file \
                 '{c}/clang/ModuleCache/1XGORMFR2JUL/SwiftShims-6PVXR3VD9JVP.pcm': \
                 'Operation not permitted'"
            ),
            Kind::SystemCache,
        ),
        (
            format!(
                "error: unable to open output file '/private{c}/clang/ModuleCache/x.pcm': \
                 'Operation not permitted'"
            ),
            Kind::SystemCache,
        ),
        (
            format!(
                "xcrun: error: couldn't create cache file '{t}/xcrun_db-5UlOOWst' \
                 (errno=Operation not permitted)"
            ),
            Kind::SystemTemp,
        ),
    ] {
        assert_eq!(
            detect_with_targets(&came_back("swiftc a.swift", "", &line), &place()),
            vec![(block(Operation::Write, kind, false), None)],
            "{line}"
        );
    }
    // The same words about a path the chat may write are not the sandbox's.
    let own =
        format!("error: unable to open output file '{CHAT}/build/x.o': 'Operation not permitted'");
    assert!(detect(&came_back("x", "", &own), &place()).is_empty());
    // Nor is a line that only quotes them in the middle.
    let quoted = format!(
        "note: saw \"couldn't create cache file '{t}/xcrun_db' (errno=Operation not \
         permitted)\" earlier"
    );
    assert!(detect(&came_back("x", "", &quoted), &place()).is_empty());
}

/// #1416: Foundation's refusal of a write, as a Swift program prints the error, names the path
/// in its `NSFilePath` and the system's words in its underlying POSIX error. Read as any other
/// program's own words: sorted by the path, and only outside what the chat may write. A line
/// that names no path says nothing a block could be sorted by, and is not one, as
/// `ps: Operation not permitted` is not.
#[test]
fn foundations_refused_write_is_read_by_the_path_it_names() {
    let foundation = |path: &str| {
        format!(
            "Fatal error: Error Domain=NSCocoaErrorDomain Code=513 \"You don’t have permission to \
             save the file “p1120.txt” in the folder “T”.\" UserInfo={{NSFilePath={path}, \
             NSUnderlyingError=0x600000c84030 {{Error Domain=NSPOSIXErrorDomain Code=1 \
             \"Operation not permitted\"}}}}"
        )
    };
    for (path, kind) in [
        (format!("{PER_USER_T}/p1120.txt"), Kind::SystemTemp),
        (format!("/private{PER_USER_T}/p1120.txt"), Kind::SystemTemp),
        (
            "/Users/dev/plane/workspaces/beta/x.txt".to_owned(),
            Kind::ProjectFiles,
        ),
    ] {
        let line = foundation(&path);
        assert_eq!(
            detect(&came_back("swift p.swift", "", &line), &place()),
            vec![block(Operation::Write, kind, false)],
            "{line}"
        );
    }
    // A path holding a comma or a brace is read whole, and a file name in the description
    // that looks like a key is not the path.
    let odd = "/Users/dev/plane/workspaces/beta/a, b}c/x.txt";
    let line = foundation(odd).replace(
        "the file “p1120.txt”",
        "the file “NSFilePath=/Users/dev/plane/workspaces/alpha/x”",
    );
    let found = detect_with_targets(&came_back("swift p.swift", "", &line), &place());
    assert_eq!(found.len(), 1, "{line}");
    assert_eq!(
        found[0].0,
        block(Operation::Write, Kind::ProjectFiles, false)
    );
    assert!(
        format!("{:?}", found[0]).contains("a, b}c"),
        "{:?}",
        found[0]
    );
    // In what the chat may write, the sandbox did not refuse it.
    let own = foundation(&format!("{CHAT}/out.txt"));
    assert!(detect(&came_back("x", "", &own), &place()).is_empty());
    // With no path, or with another underlying error, there is nothing to sort.
    for line in [
        "Error Domain=NSCocoaErrorDomain Code=513 \"You don’t have permission.\" \
         UserInfo={NSUnderlyingError=0x6 {Error Domain=NSPOSIXErrorDomain Code=1 \
         \"Operation not permitted\"}}"
            .to_owned(),
        foundation(&format!("{PER_USER_T}/p.txt")).replace(
            "NSPOSIXErrorDomain Code=1 \"Operation not permitted\"",
            "NSPOSIXErrorDomain Code=13 \"Permission denied\"",
        ),
    ] {
        assert!(
            detect(&came_back("x", "", &line), &place()).is_empty(),
            "{line}"
        );
    }
}

/// The path between a line's last two quotes, past a word's apostrophe before them.
#[test]
fn the_last_quoted_path_is_read_past_an_apostrophe() {
    assert_eq!(last_quoted("cannot copy 'a' to 'b'"), Some("b"));
    assert_eq!(
        last_quoted("couldn't create cache file '/x/y'"),
        Some("/x/y")
    );
    assert_eq!(last_quoted("failed to create directory `/x`"), Some("/x"));
    assert_eq!(last_quoted("can't open /x"), None);
}

// ---- Claude Code's proxy (#1631) --------------------------------------------------------------

/// What a command that reached three hosts through Claude Code's proxy came back with, exit 0:
/// curl printed `000` for each, and Claude Code appended its report to standard error. A refused
/// connection is said by the proxy, not Seatbelt: no process, no `deny(`, and its reason after.
const PROXY_REFUSED_THREE: &str = "https://api.example.com/health 000\n\
https://console.example.com/gateway/v1/ready 000\n\
https://mcp.example.com/health 000\n\
Shell cwd was reset to /Users/dev/plane/workspaces/alpha/repo\n\
<sandbox_violations>\n\
deny network-outbound api.example.com:443 (host is not on the allow list)\n\
deny network-outbound console.example.com:443 (host is not on the allow list)\n\
deny network-outbound mcp.example.com:443 (host is not on the allow list)\n\
</sandbox_violations>";

#[test]
fn a_host_claude_codes_proxy_refused_is_a_block_for_each_host_with_the_host_to_allow() {
    let found = detect_with_targets(&came_back("curl …", "", PROXY_REFUSED_THREE), &place());
    let host = block(Operation::Connect, Kind::Host, false);
    assert_eq!(
        found,
        vec![
            (host, Some("api.example.com:443".to_owned())),
            (host, Some("console.example.com:443".to_owned())),
            (host, Some("mcp.example.com:443".to_owned())),
        ]
    );
    // The same, from a command that failed: the block is the sandbox's own, so it is read there
    // too. And one host twice is one block.
    let twice = appended(
        "Exit code 7",
        &[
            "deny network-outbound api.example.com:443 (host is not on the allow list)",
            "deny network-outbound API.example.com:443 (host is not on the allow list)",
        ],
    );
    assert_eq!(
        detect_with_targets(&failed("curl …", &twice), &place()),
        vec![(host, Some("api.example.com:443".to_owned()))]
    );
}

#[test]
fn a_proxy_line_names_only_a_host_never_the_words_after_it() {
    // A reason Claude Code may word otherwise, and a line with none.
    for (line, target) in [
        (
            "deny network-outbound registry.example.org:443 (blocked by managed policy)",
            Some("registry.example.org:443"),
        ),
        ("deny network-outbound 10.0.0.7:6443", Some("10.0.0.7:6443")),
        // No host at all, only a reason: nothing to grant, still a refused connection.
        (
            "deny network-outbound (host is not on the allow list)",
            None,
        ),
    ] {
        assert_eq!(
            detect_with_targets(&came_back("x", "", &appended("", &[line])), &place()),
            vec![(
                block(Operation::Connect, Kind::Host, false),
                target.map(str::to_owned)
            )],
            "{line}"
        );
    }
    // A socket path is a local socket, named whole as a grant names it (ADR 0093).
    assert_eq!(
        detect_with_targets(
            &came_back(
                "x",
                "",
                &appended(
                    "",
                    &["deny network-outbound /private/tmp/x.sock (not allowed)"]
                )
            ),
            &place()
        ),
        vec![(
            block(Operation::Connect, Kind::LocalSocket, false),
            Some("/private/tmp/x.sock".to_owned())
        )]
    );
    // A client's word for the same refusal names no host: it is the one the line named, not
    // a second block.
    let both = format!(
        "curl: (56) CONNECT tunnel failed, response 403\n{}",
        appended(
            "",
            &["deny network-outbound api.example.com:443 (host is not on the allow list)"]
        )
    );
    assert_eq!(
        detect_with_targets(&came_back("curl -v x", "", &both), &place()),
        vec![(
            block(Operation::Connect, Kind::Host, false),
            Some("api.example.com:443".to_owned())
        )]
    );
}

/// A program that looks a host up itself, rather than going through the sandbox's proxy, is
/// refused the lookup by the sandbox, and macOS's resolver says only that the name is not known
/// (#1631): no violation line comes with it. Allowing the host would not let such a program
/// through, so the host it names is said and never offered (#1663,
/// `sandboxblock_network_tests`).
#[test]
fn a_lookup_the_sandbox_refused_is_a_lookup_of_a_host_with_nothing_to_grant() {
    for (line, host) in [
        (
            "psql: error: could not translate host name \"db.example.com\" to address: nodename \
             nor servname provided, or not known",
            Some("db.example.com"),
        ),
        (
            "socket.gaierror: [Errno 8] nodename nor servname provided, or not known",
            None,
        ),
        (
            "nc: getaddrinfo: nodename nor servname provided, or not known",
            None,
        ),
        (
            "curl: (6) Could not resolve host: api.example.com",
            Some("api.example.com"),
        ),
    ] {
        assert_eq!(
            detect_with_targets(&came_back("x", "", line), &place()),
            vec![(
                block(Operation::Lookup, Kind::Host, false),
                host.map(str::to_owned)
            )],
            "{line}"
        );
    }
}

/// A refused lookup is never said to be of a host the project does not allow (#1631 review):
/// the host may well be allowed, and the chat told so would ask the person to allow it, which
/// would not let the program through.
#[test]
fn a_refused_lookup_is_said_as_a_program_past_the_proxy_never_as_a_host_not_allowed() {
    let said = block(Operation::Lookup, Kind::Host, false).said();
    assert!(!said.contains("does not allow"), "{said}");
    assert!(said.contains("proxy"), "{said}");
    let told = told_the_chat(&[block(Operation::Lookup, Kind::Host, false)]).unwrap();
    assert!(!told.contains("does not allow"), "{told}");
    // A refused connection still names the project's allowlist.
    assert!(
        block(Operation::Connect, Kind::Host, false)
            .said()
            .contains("this project does not allow")
    );
}

/// Docker's client refused its socket (#1631): a local socket, named whole by its path where the
/// command failed, so its Notice can offer to allow it (ADR 0093). Docker's sentence alone
/// counts only on macOS (#1637, below).
#[test]
fn a_refused_docker_socket_is_a_connection_to_a_local_socket_it_names() {
    let alone = cfg!(target_os = "macos").then_some((
        "permission denied while trying to connect to the docker API at \
         unix:///Users/dev/.docker/run/docker.sock",
        "/Users/dev/.docker/run/docker.sock",
    ));
    for (line, socket) in alone.into_iter().chain([
        (
            "Got permission denied while trying to connect to the Docker daemon socket at \
             unix:///var/run/docker.sock: Get \"http://%2Fvar%2Frun%2Fdocker.sock/v1.47/info\": \
             dial unix /var/run/docker.sock: connect: operation not permitted",
            "/var/run/docker.sock",
        ),
        (
            "dial unix /Users/dev/.colima/default/docker.sock: connect: operation not permitted",
            "/Users/dev/.colima/default/docker.sock",
        ),
    ]) {
        // A failed command's error names it (ADR 0093, D-sock-5)…
        assert_eq!(
            detect_with_targets(&failed("docker info", line), &place()),
            vec![(
                block(Operation::Connect, Kind::LocalSocket, false),
                Some(socket.to_owned())
            )],
            "{line}"
        );
        // …a line that runs a program and came back is a block naming none, where it runs no
        // container client…
        assert_eq!(
            detect_with_targets(&came_back("make up", "", line), &place()),
            vec![(block(Operation::Connect, Kind::LocalSocket, false), None)],
            "{line}"
        );
        // …and what a line that only reads printed is none (#1972).
        assert_eq!(
            detect_with_targets(&came_back("cat build.log", "", line), &place()),
            Vec::new(),
            "{line}"
        );
    }
    // A daemon that is not running is not the sandbox.
    assert!(
        detect(
            &came_back(
                "docker info",
                "",
                "Cannot connect to the Docker daemon at unix:///var/run/docker.sock. Is the \
                 docker daemon running?"
            ),
            &place()
        )
        .is_empty()
    );
}

/// Docker's own "permission denied" sentence is the sandbox's only where the sandbox can refuse
/// a socket that way (#1637): on macOS, Seatbelt's refusal reads so. Elsewhere it is a user
/// outside the docker group, which no Start without the sandbox mends. The socket's `EPERM`
/// (Go's `connect: operation not permitted`) is the sandbox's everywhere, and `EACCES`
/// (`connect: permission denied`) is a socket's own file mode everywhere.
#[test]
fn dockers_permission_words_are_a_block_only_where_the_sandbox_can_say_them() {
    let docker_alone = "permission denied while trying to connect to the docker API at \
                        unix:///Users/dev/.docker/run/docker.sock";
    let docker_eperm = "Got permission denied while trying to connect to the Docker daemon socket \
                        at unix:///var/run/docker.sock: Get \"http://%2Fvar%2Frun%2Fdocker.sock/\
                        v1.47/info\": dial unix /var/run/docker.sock: connect: operation not \
                        permitted";
    let docker_eacces = "permission denied while trying to connect to the Docker daemon socket \
                         at unix:///var/run/docker.sock: Get \"http://%2Fvar%2Frun%2Fdocker.sock/\
                         v1.47/info\": dial unix /var/run/docker.sock: connect: permission denied";
    let go_eperm = "dial unix /Users/dev/.colima/default/docker.sock: connect: operation not \
                    permitted";
    let socket = |path: &str| Some((Operation::Connect, Kind::LocalSocket, Some(path.to_owned())));
    let desktop = socket("/Users/dev/.docker/run/docker.sock");
    let var_run = socket("/var/run/docker.sock");
    let colima = socket("/Users/dev/.colima/default/docker.sock");
    // Where the sandbox says Docker's words (macOS).
    assert_eq!(network_refusal_on(docker_alone, true), desktop);
    assert_eq!(network_refusal_on(docker_eperm, true), var_run);
    assert_eq!(network_refusal_on(go_eperm, true), colima);
    assert_eq!(network_refusal_on(docker_eacces, true), None, "a file mode");
    // Where it does not (Linux): only the socket's own `EPERM` is the sandbox's.
    assert_eq!(
        network_refusal_on(docker_alone, false),
        None,
        "the docker group"
    );
    assert_eq!(network_refusal_on(docker_eperm, false), var_run);
    assert_eq!(network_refusal_on(go_eperm, false), colima);
    assert_eq!(
        network_refusal_on(docker_eacces, false),
        None,
        "a file mode"
    );
    // This build reads them as its own platform does.
    assert_eq!(
        network_refusal(docker_alone),
        cfg!(target_os = "macos").then(|| desktop.clone().expect("a socket"))
    );
}

#[test]
fn the_chat_is_told_what_was_blocked_and_to_wait_only_when_a_notice_is_up() {
    assert_eq!(
        told_the_chat(&[]),
        None,
        "no block taken, no Notice to point at"
    );
    let host = block(Operation::Connect, Kind::Host, false);
    let told = told_the_chat(&[host, host]).expect("a block was taken");
    assert!(
        told.contains("blocked a connection to an internet host this project does not allow."),
        "{told}"
    );
    assert_eq!(
        told.matches("a connection to").count(),
        1,
        "said once: {told}"
    );
    for words in [
        "Notice",
        "this chat's tab",
        "wait for their answer",
        "do not work around",
    ] {
        assert!(told.contains(words), "{words}: {told}");
    }
    assert!(!told.contains("Report"), "{told}");
    let ours = told_the_chat(&[block(Operation::Write, Kind::ProjectFiles, true)]).unwrap();
    assert!(
        ours.contains("purlis bug") && ours.contains("Report"),
        "{ours}"
    );
}

/// #1416: no Allow is offered for macOS's per-user folders, so a chat blocked writing one is
/// told the way round in fixed words, to write under its own `$TMPDIR`, rather than to wait
/// for an answer that cannot come; any other block it was told of in the same turn still has
/// it wait. Nothing the chat ran is said.
#[test]
fn a_chat_blocked_in_the_system_temp_folder_is_told_to_write_under_its_own_tmpdir() {
    let temp = block(Operation::Write, Kind::SystemTemp, false);
    let told = told_the_chat(&[temp]).expect("a block was taken");
    for words in [
        "No Allow is offered",
        "write the file under \"$TMPDIR\"",
        "mktemp -p \"$TMPDIR\"",
    ] {
        assert!(told.contains(words), "{words}: {told}");
    }
    assert!(!told.contains("wait for their answer"), "{told}");
    assert!(!told.contains("do not retry before"), "{told}");

    let host = block(Operation::Connect, Kind::Host, false);
    let both = told_the_chat(&[temp, host]).expect("blocks were taken");
    assert!(both.contains("wait for their answer"), "{both}");
    assert!(both.contains("write the file under \"$TMPDIR\""), "{both}");

    let alone = told_the_chat(&[host]).expect("a block was taken");
    assert!(!alone.contains("TMPDIR"), "{alone}");

    // purlis's own write there is a purlis bug: the Report, not the way round.
    let ours = told_the_chat(&[block(Operation::Write, Kind::SystemTemp, true)]).unwrap();
    assert!(ours.contains("Report"), "{ours}");
    assert!(!ours.contains("No Allow is offered"), "{ours}");
}

// ---- the way out a refusal names (#1408, #1476) -----------------------------------------------

/// Spec #1330's testing rule, its second half (#1408 l32): a direct write of a file purlis
/// writes for a chat is refused, and the refusal names the command that writes it. One row per
/// brokered write; a project file nothing brokers names none.
#[test]
fn a_refused_write_of_a_brokered_file_names_the_command_that_writes_it() {
    for (file, command) in [
        ("sessions/2026-10-12-x.md", "`purlis session record`"),
        (
            "workspaces/alpha/sessions/2026-10-12-x.md",
            "`purlis session record`",
        ),
        (
            "workspaces/alpha/memory/a-note.md",
            "`purlis workspace remember -w <workspace>",
        ),
        ("workspaces/alpha/memory/MEMORY.md", "the memory_add tool"),
        (
            "personas/steward/memory/a-note.md",
            "`purlis persona remember <persona>",
        ),
        (
            "personas/_shared/memory/a-note.md",
            "(shared for shared memory)",
        ),
        ("workspaces/alpha/todos/a-todo.md", "`purlis workspace todo"),
        ("workspaces/alpha/workspace.md", "`purlis workspace vision"),
    ] {
        let path = format!("{ROOT}/{file}");
        assert_eq!(
            kind_of(Path::new(&path), &place()),
            Kind::ProjectFiles,
            "{file}"
        );
        let way = way_out(Path::new(&path), Path::new(ROOT)).expect(file);
        assert!(way.contains(command), "{file}: {way}");
    }
    for file in [
        "workspaces/alpha/notes.txt",
        "workspaces/alpha/memory",
        "personas/steward/persona.md",
        "docs/index.md",
        "sessionsx/a.md",
    ] {
        assert_eq!(
            way_out(Path::new(&format!("{ROOT}/{file}")), Path::new(ROOT)),
            None,
            "{file}"
        );
    }
    // Outside the project, nothing is brokered.
    assert_eq!(
        way_out(
            Path::new("/Users/dev/elsewhere/sessions/a.md"),
            Path::new(ROOT)
        ),
        None
    );
}

/// #1476: a branch folder keeps its index in its clone's `.git/worktrees/<name>`, which the
/// sandbox protects, so `git add` and `git commit` there are refused on its `index.lock`. The
/// block stays one on a protected file, and its path travels to the app alone, so the Notice
/// can name `purlis worktree commit`; no other protected file's path does.
#[test]
fn a_refused_write_in_a_branch_folders_git_record_names_purlis_worktree_commit() {
    let lock = "/Users/dev/plane/workspaces/alpha/repo/.git/worktrees/repo-fix/index.lock";
    let found = detect_with_targets(
        &came_back(
            "git add -A",
            "",
            &appended("", &[&format!("git(5) deny(1) file-write-create {lock}")]),
        ),
        &place(),
    );
    assert_eq!(
        found,
        vec![(
            block(Operation::Write, Kind::ProtectedFile, false),
            Some(lock.to_owned())
        )]
    );
    let way = way_out(Path::new(lock), Path::new(ROOT)).expect("a way out");
    assert!(way.contains("`purlis worktree commit"), "{way}");
    // #3831: and its fetch or rebase, the update the app makes for the chat.
    assert!(way.contains("`purlis worktree update`"), "{way}");
    // #3798: a raw `git worktree add` is refused on the same record, so the way out names the
    // command that cuts a branch folder too.
    assert!(
        way.contains("`purlis worktree add <repo> <piece>`"),
        "{way}"
    );
    // Any protected file other than a branch folder's record keeps its path to itself.
    for other in [
        "/Users/dev/plane/workspaces/alpha/repo/.git/config",
        "/Users/dev/plane/workspaces/alpha/repo/.git/hooks/pre-commit",
        "/Users/dev/plane/workspaces/alpha/repo/.vscode/settings.json",
    ] {
        let found = detect_with_targets(
            &came_back(
                "x",
                "",
                &appended("", &[&format!("x(5) deny(1) file-write-create {other}")]),
            ),
            &place(),
        );
        assert_eq!(
            found,
            vec![(block(Operation::Write, Kind::ProtectedFile, false), None)],
            "{other}"
        );
        assert_eq!(way_out(Path::new(other), Path::new(ROOT)), None, "{other}");
    }
}

/// ADR 0093 (D-sock-5): a failed command's output names the socket it was refused, as the
/// sandbox's own line does, and the certificate check names its one service.
#[test]
fn a_failed_commands_output_names_its_socket_and_the_certificate_checks_service() {
    let failed = serde_json::json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Bash",
        "tool_input": { "command": "psql" },
        "error": "psql: error: dial unix /tmp/.s.PGSQL.5432: connect: operation not permitted",
    });
    let found = detect_with_targets(&failed, &place());
    assert!(
        found.contains(&(
            block(Operation::Connect, Kind::LocalSocket, false),
            Some("/tmp/.s.PGSQL.5432".to_owned())
        )),
        "{found:?}"
    );
    let violation = came_back(
        "glab api user",
        "",
        "<sandbox_violations>\nglab(42) deny(1) mach-lookup com.apple.trustd.agent\n\
         helper(43) deny(1) mach-lookup com.example.helper\n</sandbox_violations>",
    );
    let found = detect_with_targets(&violation, &place());
    assert!(
        found.contains(&(
            block(Operation::Lookup, Kind::CertificateCheck, false),
            Some(crate::sandbox::services::TRUSTD.to_owned())
        )),
        "{found:?}"
    );
    assert!(
        found.contains(&(
            block(Operation::Lookup, Kind::SystemService, false),
            Some("com.example.helper".to_owned())
        )),
        "{found:?}"
    );
}

/// #1808's review, MF-5: a command that succeeded and printed a refusal's words (`cat` of a log,
/// `git show` of a test, a README) names no socket to allow, in either stream; only a failed
/// command's own error, or the sandbox's line, does.
#[test]
fn a_successful_commands_output_names_no_socket_to_allow() {
    let quoted = "dial unix /var/run/docker.sock: connect: operation not permitted\n\
                  docker: permission denied while trying to connect to the Docker daemon socket \
                  at unix:///var/run/docker.sock: Head \"http://x/_ping\": dial unix \
                  /var/run/docker.sock: connect: operation not permitted\n";
    for payload in [
        came_back("cat notes.log", quoted, ""),
        came_back("git show HEAD:x.rs", "", quoted),
    ] {
        let found = detect_with_targets(&payload, &place());
        assert!(
            found
                .iter()
                .all(|(block, target)| block.kind != Kind::LocalSocket || target.is_none()),
            "{found:?}"
        );
    }
}

// ---- a container client whose status the line dropped (qw338) -----------------------------------

const DESKTOP_SOCKET: &str = "/Users/dev/.docker/run/docker.sock";

/// What Docker's and Podman's clients say of a socket the sandbox refused them, each naming the
/// one socket, [`DESKTOP_SOCKET`]. Docker's sentence with no `EPERM` beside it is the sandbox's
/// on macOS alone (#1637).
fn refused_socket_words() -> Vec<String> {
    let mut said = vec![
        // docker 24 to 27, `docker run`.
        format!(
            "docker: permission denied while trying to connect to the Docker daemon socket at \
             unix://{DESKTOP_SOCKET}: Head \"http://%2FUsers%2Fdev%2F.docker%2Frun%2Fdocker.sock/\
             _ping\": dial unix {DESKTOP_SOCKET}: connect: operation not permitted.\n\
             See 'docker run --help'."
        ),
        // docker 28 and later, with the dial beside it.
        format!(
            "docker: permission denied while trying to connect to the docker API at \
             unix://{DESKTOP_SOCKET}: Head \"http://%2FUsers%2Fdev%2F.docker%2Frun%2Fdocker.sock/\
             _ping\": dial unix {DESKTOP_SOCKET}: connect: operation not permitted\n\n\
             Run 'docker run --help' for more information"
        ),
        // docker compose, and any Go client: the dial alone, after its own words.
        format!(
            "unable to get image 'golang:1.26.6': failed to connect to the docker API at \
             unix://{DESKTOP_SOCKET}; check if the path is correct and if the daemon is running: \
             dial unix {DESKTOP_SOCKET}: connect: operation not permitted"
        ),
        format!("dial unix {DESKTOP_SOCKET}: connect: operation not permitted"),
    ];
    if cfg!(target_os = "macos") {
        said.push(format!(
            "docker: permission denied while trying to connect to the docker API at \
             unix://{DESKTOP_SOCKET}\n\nRun 'docker run --help' for more information"
        ));
    }
    said
}

fn named_socket() -> Vec<(Block, Option<String>)> {
    vec![(
        block(Operation::Connect, Kind::LocalSocket, false),
        Some(DESKTOP_SOCKET.to_owned()),
    )]
}

fn unnamed_socket() -> Vec<(Block, Option<String>)> {
    vec![(block(Operation::Connect, Kind::LocalSocket, false), None)]
}

/// A `docker run` that failed, as Claude Code hands it over: every shape of the client's words
/// names the socket.
#[test]
fn a_failed_docker_run_names_its_socket_in_each_of_the_clients_words() {
    let command = "docker run --rm -v \"$PWD/api\":/src -w /src golang:1.26.6 go build ./...";
    for said in refused_socket_words() {
        assert_eq!(
            detect_with_targets(
                &failed(command, &format!("Exit code 125\n{said}")),
                &place()
            ),
            named_socket(),
            "{said}"
        );
    }
}

/// The operator's case, replayed (qw338): a chat ran `docker run … 2>&1 | tail`, the sandbox
/// refused Docker its socket, and the pipe's `tail` ended with status 0. Claude Code handed the
/// result over as one that came back, with Docker's words on standard output and no failure
/// beside them, and the Notice named no socket: Start without the sandbox, and no Allow.
#[test]
fn a_docker_run_whose_status_its_line_dropped_still_names_its_socket() {
    let run = "docker run --rm -v \"$PWD/api\":/src -w /src golang:1.26.6 sh -c 'go build -o \
               /tmp/plt ./cmd/platform-loadtest && /tmp/plt -help'";
    for said in refused_socket_words() {
        // Both streams down a pipe: standard output.
        for command in [
            format!("{run} 2>&1 | tail -40"),
            format!("cd api && {run} 2>&1 | tail -40"),
            format!("DOCKER_BUILDKIT=1 timeout 300 {run} 2>&1 | tee build.log | tail -5"),
            format!("/usr/local/bin/{run} 2>&1 | tail"),
        ] {
            assert_eq!(
                detect_with_targets(&came_back(&command, &format!("{said}\n"), ""), &place()),
                named_socket(),
                "{command}\n{said}"
            );
        }
        // A status the line dropped another way: Docker's words stay on standard error.
        for command in [format!("{run}; echo \"exit=$?\""), format!("{run} || true")] {
            assert_eq!(
                detect_with_targets(&came_back(&command, "exit=125\n", &said), &place()),
                named_socket(),
                "{command}\n{said}"
            );
        }
    }
    // Every container client purlis knows, by its name alone.
    let said = format!("dial unix {DESKTOP_SOCKET}: connect: operation not permitted\n");
    for command in [
        "docker compose up -d 2>&1 | tail -20",
        "docker-compose up -d 2>&1 | tail -20",
        "podman ps 2>&1 | head",
        "nerdctl ps 2>&1 | head",
    ] {
        assert_eq!(
            detect_with_targets(&came_back(command, &said, ""), &place()),
            named_socket(),
            "{command}"
        );
    }
}

/// MF-5 holds: what a command that ran no container client printed names no socket, wherever
/// the word `docker` stands in its line. A file, a page or a log can say any socket, and none
/// of them can make its reader fail: a line that only reads raises no block at all (#1972).
#[test]
fn a_line_that_runs_no_container_client_names_no_socket_whatever_it_printed() {
    let readers = [
        "cat docker.log",
        "cat notes.log | tail -40",
        "grep -r docker . | tail",
        "git show HEAD:docs/docker.md",
        "echo 'docker run x; docker ps' && cat notes.log",
        "",
    ];
    let programs = [
        "curl -s https://example.com/docker | head",
        "sh -c 'docker run x' 2>&1 | tail",
        "make up 2>&1 | tail",
        "dockerize -version; cat notes.log",
    ];
    for said in refused_socket_words() {
        for (command, raised) in readers
            .iter()
            .map(|command| (command, Vec::new()))
            .chain(programs.iter().map(|command| (command, unnamed_socket())))
        {
            for payload in [
                came_back(command, &format!("{said}\n"), ""),
                came_back(command, "", &said),
            ] {
                assert_eq!(
                    detect_with_targets(&payload, &place()),
                    raised,
                    "{command}\n{said}"
                );
            }
        }
    }
    // A result with no command beside it, or one that is no text, raises none either.
    let said = format!("dial unix {DESKTOP_SOCKET}: connect: operation not permitted\n");
    for input in [json!({}), json!({"command": ["docker", "ps"]}), json!(null)] {
        let payload = json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Bash",
            "tool_input": input,
            "tool_response": {"stdout": said, "stderr": "", "interrupted": false},
        });
        assert_eq!(detect_with_targets(&payload, &place()), Vec::new());
    }
}

/// A container client's line names a socket from what it printed, and the certificate check by
/// its own rule, as any line that runs a program does (qw353): `docker pull` checks one too.
#[test]
fn a_container_clients_line_names_its_socket_and_the_certificate_check() {
    let command = "docker pull example.com/x 2>&1 | tail";
    let found = detect_with_targets(
        &came_back(
            command,
            "Get \"https://example.com/v2/\": tls: failed to verify certificate: x509: OSStatus \
             -26276\n",
            "",
        ),
        &place(),
    );
    assert_eq!(
        found,
        vec![(
            block(Operation::Lookup, Kind::CertificateCheck, false),
            Some(crate::sandbox::services::TRUSTD.to_owned())
        )]
    );
}

/// A daemon that is not running is not the sandbox's refusal, failed or not: Docker says so of
/// a socket nothing listens on, and of one that is not there.
#[test]
fn a_docker_daemon_that_is_not_running_is_no_block() {
    let said = format!(
        "Cannot connect to the Docker daemon at unix://{DESKTOP_SOCKET}. Is the docker daemon \
         running?"
    );
    for payload in [
        failed("docker ps", &format!("Exit code 1\n{said}")),
        came_back("docker ps 2>&1 | tail", &said, ""),
        came_back("docker ps || true", "", &said),
    ] {
        assert_eq!(detect_with_targets(&payload, &place()), Vec::new());
    }
}

/// Codex and opencode hand a command that did not fail over as one string, and its line is read
/// the same way: a container client's own words name its socket, and a violation line in the
/// string, which the command may have printed, still names nothing.
#[test]
fn a_container_clients_one_string_result_names_its_socket_and_no_violation_line_does() {
    let said = format!("dial unix {DESKTOP_SOCKET}: connect: operation not permitted\n");
    assert_eq!(
        detect_with_targets(&codex_came_back("docker ps 2>&1 | tail", &said), &place()),
        named_socket()
    );
    assert_eq!(
        detect_with_targets(&codex_came_back("cat notes.log", &said), &place()),
        Vec::new()
    );
    let printed = appended(
        "ok",
        &["nc(11) deny(1) network-outbound /Users/dev/.ssh/agent.sock"],
    );
    assert_eq!(
        detect_with_targets(&codex_came_back("docker ps", &printed), &place()),
        unnamed_socket()
    );
}

// ---- Codex's own block hook (#1663, #1683) ------------------------------------------------------

/// A `PostToolUse` payload as codex-cli 0.147.0 hands it for its shell tool: named `Bash`
/// (`codex-rs/core/src/tools/hook_names.rs`, `HookToolName::bash`), the command at
/// `tool_input.command`, and what the command printed on both streams as one string at
/// `tool_response` (`handlers/shell.rs`, `context.rs`'s unified exec output).
fn codex_came_back(command: &str, printed: &str) -> serde_json::Value {
    json!({
        "session_id": "019a-codex",
        "turn_id": "turn-1",
        "transcript_path": null,
        "cwd": CHAT,
        "hook_event_name": "PostToolUse",
        "model": "gpt-5.1-codex",
        "permission_mode": "default",
        "tool_name": "Bash",
        "tool_use_id": "call_1",
        "tool_input": {"command": command},
        "tool_response": printed,
    })
}

/// A Codex chat's lookups, refused sockets and printed refusals are read from its own hook,
/// as standard output is: a lookup names the host it was of (shown, never offered), a socket
/// names nothing to allow from a command's output, and a refused host is left to purlis's own
/// proxy, which tells it by name.
#[test]
fn a_codex_shell_result_is_read_for_the_networks_refusals() {
    let printed = "fetching\n\
        curl: (6) Could not resolve host: api.example.com\n\
        Cannot connect to the Docker daemon: dial unix /Users/dev/.docker/run/docker.sock: \
        connect: operation not permitted\n\
        purlis's sandbox does not allow x.example:443: no egress preset or host of this project \
        lists it\n";
    let found = detect_with_targets(&codex_came_back("make fetch", printed), &place());
    assert_eq!(
        found,
        vec![
            (
                block(Operation::Lookup, Kind::Host, false),
                Some("api.example.com".to_owned())
            ),
            (block(Operation::Connect, Kind::LocalSocket, false), None),
            (block(Operation::Connect, Kind::Host, false), None),
        ]
    );
    // The host the proxy refused is the proxy's to tell, so one refusal is one Notice.
    assert_eq!(
        the_hooks_own(found, Some("codex"), crate::sandbox::Os::MacOs),
        vec![
            (
                block(Operation::Lookup, Kind::Host, false),
                Some("api.example.com".to_owned())
            ),
            (block(Operation::Connect, Kind::LocalSocket, false), None),
        ]
    );
}

/// A block is named only from a failed command: a violation line in one string, which a Codex
/// command or an opencode command that did not fail hands over, may be one the command printed,
/// so it names no host, socket or service to allow. (A line that runs a program names the
/// certificate check, by its own rule: qw353.)
#[test]
fn a_violation_line_in_a_result_that_did_not_fail_names_nothing_to_allow() {
    let printed = appended(
        "ok",
        &[
            "nc(11) deny(1) network-outbound /Users/dev/.docker/run/docker.sock",
            "gh(8) deny(1) mach-lookup com.apple.trustd.agent",
            "curl(10) deny(1) network-outbound 93.184.216.34:443",
        ],
    );
    let opencode = json!({"tool_name": "Bash", "tool_input": {"command": "cat x.log"}, "tool_response": printed});
    for payload in [codex_came_back("cat x.log", &printed), opencode] {
        let found = detect_with_targets(&payload, &place());
        assert!(!found.is_empty(), "the blocks are still raised");
        assert!(
            found.iter().all(|(_, target)| target.is_none()),
            "{found:?}"
        );
    }
}

/// What a Codex command printed is its own text: a line that only quotes a refusal's words, and
/// a program's own "Operation not permitted" on a path, are no block there.
#[test]
fn a_codex_result_that_quotes_a_refusal_or_names_a_path_is_no_block() {
    for printed in [
        "grep: the words \"Could not resolve host: api.example.com\" appear in notes.md\n",
        "mkdir: /opt/tool: Operation not permitted\n",
        "",
    ] {
        assert_eq!(
            detect_with_targets(&codex_came_back("x", printed), &place()),
            Vec::new(),
            "{printed}"
        );
    }
}

/// #1408 (D-1408-2's other half): a chat that makes a new workspace by hand is refused the
/// workspace's folder, or its workspace.md, and the refusal names `purlis workspace create`,
/// which purlis brokers. No name is put in the words: the Notice keeps no path.
#[test]
fn a_refused_new_workspace_names_purlis_workspace_create() {
    for file in ["workspaces/gamma", "workspaces/gamma/workspace.md"] {
        let path = format!("{ROOT}/{file}");
        assert_eq!(
            kind_of(Path::new(&path), &place()),
            Kind::ProjectFiles,
            "{file}"
        );
        let way = way_out(Path::new(&path), Path::new(ROOT)).expect(file);
        assert!(
            way.contains("`purlis workspace create <name> --vision \"<text>\"`"),
            "{file}: {way}"
        );
        assert!(!way.contains("gamma"), "{way}");
    }
    // An existing workspace's workspace.md still names its vision.
    let md = format!("{ROOT}/workspaces/alpha/workspace.md");
    assert!(
        way_out(Path::new(&md), Path::new(ROOT))
            .is_some_and(|way| way.contains("`purlis workspace vision")),
    );
    // The workspaces folder itself, and anything below a workspace that purlis does not write,
    // keep their own way out.
    for other in ["workspaces", "workspaces/gamma/notes.txt"] {
        assert_eq!(
            way_out(Path::new(&format!("{ROOT}/{other}")), Path::new(ROOT)),
            None,
            "{other}"
        );
    }
}

// ---- a tool that ran nothing (#1884) -------------------------------------------------------

#[test]
fn a_read_and_a_background_outputs_reader_ran_nothing_and_a_shell_always_did() {
    for tool in ["Read", "BashOutput"] {
        assert!(
            read_back(&serde_json::json!({"tool_name": tool, "tool_response": {}})),
            "{tool}"
        );
    }
    for tool in ["Bash", "bash", "Write", "Edit", ""] {
        assert!(
            !read_back(&serde_json::json!({
                "tool_name": tool,
                "tool_input": {"command": "cat docker.log"},
            })),
            "{tool}"
        );
    }
    assert!(!read_back(&serde_json::json!({})));
}

#[test]
fn a_chat_is_not_told_to_wait_on_what_it_only_read() {
    let socket = Block {
        operation: Operation::Connect,
        kind: Kind::LocalSocket,
        ours: false,
    };
    assert_eq!(told_the_chat_of_a_read(&[]), None);
    let told = told_the_chat_of_a_read(&[socket, socket]).expect("told");
    assert!(told.starts_with("⬢ What you just read holds"), "{told}");
    assert_eq!(told.matches("a connection to a local socket").count(), 1);
    assert!(told.contains("nothing waits on their answer"), "{told}");
    assert!(!told.contains("wait for their answer"), "{told}");
    // A command it ran is told to wait, as ever.
    let ran = told_the_chat(&[socket]).expect("told");
    assert!(ran.contains("then wait for their answer"), "{ran}");
}

#[test]
fn a_command_refused_after_its_refusal_was_read_back_is_heard_on_its_own() {
    let socket = Block {
        operation: Operation::Connect,
        kind: Kind::LocalSocket,
        ours: false,
    };
    let now = std::time::Instant::now();
    let mut throttle = Throttle::default();
    assert!(throttle.lets_on_read(4, &socket, None, now));
    assert!(!throttle.lets_on_read(4, &socket, None, now), "read again");
    // The command itself, refused within the same minute: its own block, let through.
    assert!(throttle.lets_on(4, &socket, None, now));
    assert!(!throttle.lets_on(4, &socket, None, now), "and once");
}

#[test]
fn a_refusal_read_back_from_a_background_commands_output_is_no_block() {
    let said = "docker: permission denied while trying to connect to the docker API at \
                unix:///Users/dev/.docker/run/docker.sock: dial unix \
                /Users/dev/.docker/run/docker.sock: connect: operation not permitted\n";
    let payload = json!({
        "tool_name": "Read",
        "tool_input": {"file_path": "/private/tmp/claude-501/x/tasks/b1.output"},
        "tool_response": {"file": {"content": said}},
    });
    let place = Place {
        root: Path::new("/Users/dev/plane"),
        chat: Path::new("/Users/dev/plane/workspaces/a/repo"),
        cwd: Path::new("/Users/dev/plane/workspaces/a/repo"),
        home: Some(Path::new("/Users/dev")),
    };
    // Nothing beside it says how the command ended (#1972).
    assert!(read_back(&payload));
    assert_eq!(detect_with_targets(&payload, &place), Vec::new());
}

// ---- the certificate check, whose one service is purlis's own word (qw353) ----------------------

/// What `gh` and `glab` say of a certificate check the sandbox refused them.
const REFUSED_CHECK: &str = "Post \"https://api.github.com/graphql\": tls: failed to verify \
     certificate: x509: OSStatus -26276";

fn named_check() -> Vec<(Block, Option<String>)> {
    vec![(
        block(Operation::Lookup, Kind::CertificateCheck, false),
        Some(crate::sandbox::services::TRUSTD.to_owned()),
    )]
}

fn unnamed_check() -> Vec<(Block, Option<String>)> {
    vec![(
        block(Operation::Lookup, Kind::CertificateCheck, false),
        None,
    )]
}

/// The operator's case, replayed (qw353): a chat ran `git status -sb | head -3 && gh pr list …
/// 2>&1 | head -40`, the sandbox refused `gh` the certificate check, and the pipe's `head`
/// ended with status 0. The Notice said it was the certificate check and that there was no
/// service to allow: Start without the sandbox, and no Allow. The certificate check is one
/// service, and its name is purlis's own, so a line that runs a program names it however the
/// line ended.
#[test]
fn a_certificate_check_whose_status_its_line_dropped_still_names_its_service() {
    let list = "gh pr list --state all --search \"head:purlis/save\" --limit 5 --json \
                number,title,state,headRefName,url";
    // Both streams down a pipe: standard output, after what the line's other programs printed.
    for command in [
        format!("git status -sb | head -3 && {list} 2>&1 | head -40"),
        format!("{list} 2>&1 | head -40"),
        format!("cd repo && GH_PAGER= timeout 60 {list} 2>&1 | tee pr.log | tail -5"),
        format!("/opt/homebrew/bin/{list} 2>&1 | cat"),
        "glab mr list 2>&1 | head".to_owned(),
        "make release 2>&1 | tail -20".to_owned(),
        "for n in 1 2; do gh pr view \"$n\"; done 2>&1 | head".to_owned(),
        "if ! gh auth token >/dev/null; then echo no; fi; gh api user | jq .login".to_owned(),
        format!("bash -lc '{list} 2>&1 | head -40'"),
    ] {
        assert_eq!(
            detect_with_targets(
                &came_back(
                    &command,
                    &format!("## main...origin/main\n{REFUSED_CHECK}\n"),
                    ""
                ),
                &place()
            ),
            named_check(),
            "{command}"
        );
    }
    // A status the line dropped another way: the words stay on standard error.
    for command in [
        format!("{list}; echo \"exit=$?\""),
        format!("{list} || true"),
        format!("{list} || echo failed"),
    ] {
        assert_eq!(
            detect_with_targets(&came_back(&command, "exit=1\n", REFUSED_CHECK), &place()),
            named_check(),
            "{command}"
        );
    }
    // Codex and opencode hand over one string, failed or not.
    for command in [
        format!("{list} 2>&1 | head -40"),
        list.to_owned(),
        format!("{list} || true"),
    ] {
        assert_eq!(
            detect_with_targets(
                &codex_came_back(&command, &format!("{REFUSED_CHECK}\n")),
                &place()
            ),
            named_check(),
            "{command}"
        );
    }
}

/// MF-5 holds for the certificate check: a result that only read the words raises no block
/// (#1972). A line made of readers alone, a string of readers a shell is handed, a file tool's
/// read, and a result with no command beside it.
#[test]
fn a_line_that_only_reads_names_no_certificate_check_whatever_it_printed() {
    let read = format!("./gh.log:{REFUSED_CHECK}\n");
    for command in [
        "cat gh.log",
        "grep -r OSStatus .",
        "grep -rn OSStatus . | head -40",
        "cd logs && cat gh.log | tail -40",
        "git show HEAD:docs/gh.md",
        "git -C repo log -p | head -200",
        "rg OSStatus; echo done",
        "sh -c 'cat gh.log'",
        "bash -lc \"grep -r OSStatus . | head\"",
        "echo \"$(cat gh.log)\"",
        "LC_ALL=C sed -n 1,40p gh.log",
        "for f in *.log; do cat \"$f\"; done",
        "if grep -q OSStatus gh.log; then cat gh.log; fi",
        "while read -r line; do echo \"$line\"; done < gh.log",
        "{ cat gh.log; } | head",
        "(cd logs && cat gh.log)",
        "case \"$1\" in a) cat gh.log ;; esac",
        "X=1",
        "",
    ] {
        for payload in [
            came_back(command, &read, ""),
            came_back(command, "", &read),
            codex_came_back(command, &read),
        ] {
            assert_eq!(
                detect_with_targets(&payload, &place()),
                Vec::new(),
                "{command}"
            );
        }
    }
    // A result with no command beside it, or one that is no text: `BashOutput`'s, a list.
    for (tool, input) in [
        ("BashOutput", json!({"bash_id": "b1"})),
        ("Bash", json!({})),
        ("Bash", json!({"command": ["gh", "pr", "list"]})),
        ("Bash", json!(null)),
    ] {
        let payload = json!({
            "hook_event_name": "PostToolUse",
            "tool_name": tool,
            "tool_input": input,
            "tool_response": {"stdout": read, "stderr": "", "interrupted": false},
        });
        assert_eq!(
            detect_with_targets(&payload, &place()),
            Vec::new(),
            "{tool}"
        );
    }
    // A file tool's read of a background command's output, and of any other file.
    let background = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Read",
        "tool_input": {"file_path": "/private/tmp/claude-502/x/tasks/b1.output"},
        "tool_response": {"file": {"content": read}},
    });
    assert_eq!(detect_with_targets(&background, &place()), Vec::new());
    let file = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Read",
        "tool_input": {"file_path": "/Users/dev/plane/gh.log", "command": "gh pr list"},
        "tool_response": {"file": {"content": read}},
    });
    assert_eq!(detect_with_targets(&file, &place()), Vec::new());
    // A line too big to read, and one that does not tokenize, name no service; they may have
    // run a program, so the block is still raised (#1972).
    let big = format!(
        "gh pr list # {}",
        "x".repeat(crate::guardcaps::MAX_COMMAND_BYTES)
    );
    for command in [big.as_str(), "gh pr list 'unclosed"] {
        assert_eq!(
            detect_with_targets(&came_back(command, &read, ""), &place()),
            unnamed_check()
        );
    }
}

/// The service a line that runs a program names is the certificate check's and no other: a
/// socket, a host and another service keep to their own rules, and purlis's own lookup is a
/// purlis bug, never a grant.
#[test]
fn a_line_that_runs_a_program_names_the_certificate_check_and_nothing_else() {
    let printed = appended(
        "ok",
        &[
            "nc(11) deny(1) network-outbound /Users/dev/.docker/run/docker.sock",
            "gh(8) deny(1) mach-lookup com.apple.trustd.agent",
            "helper(9) deny(1) mach-lookup com.example.helper",
            "curl(10) deny(1) network-outbound 93.184.216.34:443",
        ],
    );
    let found = detect_with_targets(&codex_came_back("gh pr list", &printed), &place());
    assert_eq!(found.len(), 4, "{found:?}");
    for (block, target) in &found {
        assert_eq!(
            target.as_deref(),
            (block.kind == Kind::CertificateCheck).then_some(crate::sandbox::services::TRUSTD),
            "{block:?}"
        );
    }
    let ours = appended(
        "ok",
        &["purlis(8) deny(1) mach-lookup com.apple.trustd.agent"],
    );
    assert_eq!(
        detect_with_targets(&codex_came_back("purlis save", &ours), &place()),
        vec![(block(Operation::Lookup, Kind::CertificateCheck, true), None)]
    );
}

/// `gh auth status` under a refused certificate check (qw353): it exits 1 and says only that
/// the token is invalid, with none of the check's words. Where the harness appended the
/// sandbox's own line, that names the check, pipe or no pipe. Where it did not, nothing in the
/// result says the sandbox refused anything, and no block is raised: a known dead end, since
/// the same words are what a token that is really invalid gets.
#[test]
fn gh_auth_status_names_the_certificate_check_only_by_the_sandboxs_own_line() {
    let said = "github.com\n  X Failed to log in to github.com account dev (keyring)\n  - \
                Active account: true\n  - The token in keyring is invalid.\n  - To \
                re-authenticate, run: gh auth login -h github.com";
    let line = "gh(8) deny(1) mach-lookup com.apple.trustd.agent";
    for payload in [
        failed(
            "gh auth status",
            &appended(&format!("Exit code 1\n{said}"), &[line]),
        ),
        came_back("gh auth status 2>&1 | head", said, &appended("", &[line])),
        came_back("gh auth status || true", "", &appended(said, &[line])),
        codex_came_back("gh auth status", &appended(said, &[line])),
    ] {
        assert_eq!(detect_with_targets(&payload, &place()), named_check());
    }
    for payload in [
        failed("gh auth status", &format!("Exit code 1\n{said}")),
        came_back("gh auth status 2>&1 | head", said, ""),
        codex_came_back("gh auth status", said),
    ] {
        assert_eq!(detect_with_targets(&payload, &place()), Vec::new());
    }
}

/// The line is read once, in time linear in its length: many programs, and many readers.
#[test]
fn a_long_line_is_read_for_what_it_runs_in_linear_time() {
    crate::lineartime::reads_in_linear_time(1 << 9, |programs| {
        let readers: String = (0..programs)
            .map(|i| format!("cat f{i} | head; "))
            .collect();
        assert_eq!(
            detect_with_targets(
                &came_back(&readers, &format!("{REFUSED_CHECK}\n"), ""),
                &place()
            ),
            Vec::new()
        );
        // Here-documents, each with a body of lines that look like commands.
        let bodies: String = (0..programs)
            .map(|i| format!("cat <<E{i}\ngh pr list\nmake f{i}\nE{i}\n"))
            .collect();
        assert_eq!(
            detect_with_targets(
                &came_back(&bodies, &format!("{REFUSED_CHECK}\n"), ""),
                &place()
            ),
            Vec::new()
        );
        let nested: String = (0..programs)
            .map(|i| format!("sh -c 'cat f{i} | head'; "))
            .collect();
        assert_eq!(
            detect_with_targets(
                &came_back(
                    &format!("{nested}gh pr list"),
                    &format!("{REFUSED_CHECK}\n"),
                    ""
                ),
                &place()
            ),
            named_check()
        );
    });
}

/// A here-document's body is text its opener reads, never a line the shell runs (review of
/// qw353): a chat that prints the check's words back through one names no service, whatever the
/// body's lines look like. The line around the body is read as it stands, so a program beside
/// a here-document still names the check.
#[test]
fn a_here_documents_body_is_read_as_text_and_never_as_a_program() {
    let read = format!("{REFUSED_CHECK}\n");
    for command in [
        format!("cat <<EOF\ngh pr list\n{REFUSED_CHECK}\nEOF"),
        format!("cat <<'EOF' | tee notes.md\nThe run said:\n{REFUSED_CHECK}\nEOF"),
        format!("cat <<-EOF\n\tmake release\n\t{REFUSED_CHECK}\n\tEOF"),
        format!("cat <<A <<'B'\ngh pr list\nA\n{REFUSED_CHECK}\nB\nhead notes.md"),
        format!("cat <<EOF\n{REFUSED_CHECK}\ngh pr list"),
    ] {
        for payload in [
            came_back(&command, &read, ""),
            codex_came_back(&command, &read),
        ] {
            assert_eq!(
                detect_with_targets(&payload, &place()),
                Vec::new(),
                "{command}"
            );
        }
    }
    for command in [
        "gh pr create --title x --body \"$(cat <<'EOF'\nA body.\nEOF\n)\" 2>&1 | tail -3",
        "cat <<'EOF' > q.graphql\nquery { viewer { login } }\nEOF\ngh api graphql -F query=@q.graphql | head",
        "bash <<'EOF'\ngh pr list\nEOF",
        "cat <<'EOF' | sh\ngh pr list\nEOF",
    ] {
        assert_eq!(
            detect_with_targets(&came_back(command, &read, ""), &place()),
            named_check(),
            "{command}"
        );
    }
}

/// The shell's own tests and loop words run nothing (review of qw353): a line of readers that
/// asks first whether the file is there still only reads.
#[test]
fn a_line_of_readers_behind_the_shells_own_test_names_no_certificate_check() {
    let read = format!("{REFUSED_CHECK}\n");
    for command in [
        "[[ -f gh.log ]] && cat gh.log",
        "for f in *.log; do [[ -s \"$f\" ]] || continue; cat \"$f\"; done",
        "while read -r f; do [[ -n \"$f\" ]] || break; cat \"$f\"; done < list",
    ] {
        assert_eq!(
            detect_with_targets(&came_back(command, &read, ""), &place()),
            Vec::new(),
            "{command}"
        );
    }
    assert_eq!(
        detect_with_targets(
            &came_back("[[ -f go.mod ]] && gh pr list | head", &read, ""),
            &place()
        ),
        named_check()
    );
}

/// One rule for a result that holds both streams in one text (review of qw353): what a line of
/// readers printed names no certificate check there either; the sandbox's own line, and a line
/// that runs a program, do.
#[test]
fn a_result_of_both_streams_in_one_text_names_the_check_by_the_same_rule() {
    let read = format!("{REFUSED_CHECK}\n");
    let both = |command: &str, text: &str| {
        json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "bash",
            "tool_input": {"command": command},
            "tool_response": {"stdout": "", "stderr": text, "mixed": true},
        })
    };
    assert_eq!(
        detect_with_targets(&both("cat gh.log", &read), &place()),
        unnamed_check()
    );
    assert_eq!(
        detect_with_targets(&both("gh pr list | head", &read), &place()),
        named_check()
    );
    let own = appended("ok", &["gh(8) deny(1) mach-lookup com.apple.trustd.agent"]);
    assert_eq!(
        detect_with_targets(&both("cat gh.log", &own), &place()),
        named_check()
    );
}

/// #2428: one failed lookup is one Notice, whether a hook or the wrap's supervisor told it
/// first: a lookup that names no host is the same as any lookup of that chat in its window.
/// Lookups of two hosts are two.
#[test]
fn a_lookup_that_names_no_host_is_one_with_any_lookup_of_its_chat_in_the_window() {
    let lookup = block(Operation::Lookup, Kind::Host, false);
    let now = std::time::Instant::now();
    let mut throttle = Throttle::default();
    assert!(throttle.lets_on(3, &lookup, None, now));
    assert!(!throttle.lets_on(3, &lookup, Some("api.example.com"), now));
    assert!(throttle.lets_on(4, &lookup, Some("api.example.com"), now));
    assert!(!throttle.lets_on(4, &lookup, None, now));
    assert!(throttle.lets_on(4, &lookup, Some("db.example.com"), now));
    // A host and a socket are no lookup: as they were.
    let host = block(Operation::Connect, Kind::Host, false);
    assert!(throttle.lets_on(3, &host, None, now));
    assert!(throttle.lets_on(3, &host, Some("api.example.com:443"), now));
}

/// #3861: a write the sandbox refused in a branch folder's git record is the chat's own to fix
/// (a line the guard could not place, `cd -`, ran `git commit` there). It is told to the chat
/// alone, with the shape to retry, and says nothing of a Notice or of waiting on the person.
/// Any other block, purlis's own one there included, keeps its Notice.
#[test]
fn a_block_in_a_branch_folders_git_record_is_the_chats_own_to_retry() {
    let lock = "/Users/dev/plane/workspaces/alpha/repo/.git/worktrees/repo-fix/index.lock";
    let own = block(Operation::Write, Kind::ProtectedFile, false);
    let way = its_own_to_retry(&own, Some(lock)).expect("the chat's own");
    assert!(way.contains("`purlis worktree commit"), "{way}");
    assert!(
        its_own_to_retry(
            &block(Operation::File, Kind::ProtectedFile, false),
            Some(lock)
        )
        .is_some()
    );

    for (other, target) in [
        // purlis's own write there is a purlis bug, reported by the person.
        (
            block(Operation::Write, Kind::ProtectedFile, true),
            Some(lock),
        ),
        // A protected file whose path never travels, or one that names no branch folder.
        (own, None),
        (
            own,
            Some("/Users/dev/plane/workspaces/alpha/repo/.git/config"),
        ),
        (own, Some("repo/.git/worktrees/repo-fix/index.lock")),
        (
            block(Operation::Read, Kind::ProtectedFile, false),
            Some(lock),
        ),
        (
            block(Operation::Write, Kind::ProjectFiles, false),
            Some(lock),
        ),
    ] {
        assert_eq!(
            its_own_to_retry(&other, target),
            None,
            "{other:?} {target:?}"
        );
    }

    assert_eq!(told_the_chat_of_its_own(&[]), None);
    let told = told_the_chat_of_its_own(&[way, way]).expect("told");
    assert_eq!(told.matches("`purlis worktree commit").count(), 1, "{told}");
    assert!(told.contains("Nobody else was told"), "{told}");
    assert!(!told.contains("Notice"), "{told}");
    assert!(!told.contains("wait"), "{told}");
}
