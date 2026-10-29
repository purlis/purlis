// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A denial the sandbox itself reported becomes a block that names exactly what was refused,
//! whatever the refused program printed or ended with.

use super::*;
use crate::sandboxblock::{detect, detect_with_targets};
use serde_json::json;
use std::path::Path;

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

/// A result that came back, as Claude Code hands it to a hook: the program's two streams as
/// one in `stdout`, and in `stderr` only the harness's own words, the report last.
fn came_back(command: &str, printed: &str, lines: &[&str]) -> serde_json::Value {
    json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "tool_response": {"stdout": printed, "stderr": report(lines), "interrupted": false},
    })
}

/// A result that failed: what was printed, then the report, in one text.
fn failed(command: &str, printed: &str, lines: &[&str]) -> serde_json::Value {
    json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "error": format!("Exit code 1\n{printed}\n{}", report(lines)),
    })
}

fn report(lines: &[&str]) -> String {
    format!(
        "<sandbox_violations>\n{}\n</sandbox_violations>",
        lines.join("\n")
    )
}

fn named(operation: Operation, kind: Kind, target: &str) -> (Block, Option<String>) {
    (
        Block {
            operation,
            kind,
            ours: false,
        },
        Some(target.to_owned()),
    )
}

// ---- one line, read ---------------------------------------------------------------------------

#[test]
fn a_kernel_line_says_the_process_the_operation_and_the_exact_target() {
    let cases = [
        (
            "docker(733) deny(1) network-outbound /private/var/run/docker.sock",
            "docker",
            "network-outbound",
            "/private/var/run/docker.sock",
        ),
        (
            "gh(4021) deny(1) mach-lookup com.apple.trustd.agent",
            "gh",
            "mach-lookup",
            "com.apple.trustd.agent",
        ),
        // A path with spaces is its whole path.
        (
            "touch(88) deny(1) file-write-create /Users/dev/Library/Application Support/x/y.db",
            "touch",
            "file-write-create",
            "/Users/dev/Library/Application Support/x/y.db",
        ),
        // As the unified log gives it: macOS's own prefix, and its count of repeats.
        (
            "Sandbox: cargo(31) deny(1) file-write-create /Users/dev/.cargo/registry/x",
            "cargo",
            "file-write-create",
            "/Users/dev/.cargo/registry/x",
        ),
        (
            "2 duplicate reports for Sandbox: node(9) deny(1) file-read-data /etc/master.passwd",
            "node",
            "file-read-data",
            "/etc/master.passwd",
        ),
    ];
    for (line, process, operation, target) in cases {
        let denial = Denial::of_line(line).unwrap_or_else(|| panic!("{line}"));
        assert_eq!(denial.witness, Witness::Kernel, "{line}");
        assert_eq!(denial.process, Some(process), "{line}");
        assert_eq!(denial.operation, operation, "{line}");
        assert_eq!(denial.target, target, "{line}");
    }
}

#[test]
fn a_proxys_line_names_its_host_and_no_process() {
    let denial = Denial::of_line(
        "deny network-outbound api.example.com:443 (host is not on the allow list)",
    )
    .unwrap();
    assert_eq!(denial.witness, Witness::Proxy);
    assert_eq!(denial.process, None);
    assert_eq!(denial.operation, "network-outbound");
    assert_eq!(denial.target, "api.example.com:443");
    // A line whose reason stands where a host would names none.
    let bare = Denial::of_line("deny network-outbound (user denied)").unwrap();
    assert_eq!(bare.target, "");
}

#[test]
fn a_line_that_is_no_denial_is_no_record() {
    for line in [
        "",
        "docker: permission denied while trying to connect",
        "allow(1) file-read-data /etc/hosts",
        "cargo(1) deny(1)",
        "deny",
        // The proxy's request-level lines say a request its policy refused, not a host to allow.
        "deny http-request GET https://api.example.com/v1/x (user denied)",
        "deny socks-tunnel api.example.com:22 (refuseOpaqueTunnels)",
        // A proxy's line begins with its `deny`: words before it make it a sentence, not a line.
        "the proxy would deny network-outbound api.example.com:443 (not allowed)",
    ] {
        assert!(Denial::of_line(line).is_none(), "{line}");
    }
}

/// The kernel puts a target that holds spaces in quotes on some lines: the record's target is
/// the path alone. A line that names no target is a record with an empty one, sorted as the
/// system's and naming nothing, never as a file in the folder the command ran in.
#[test]
fn a_quoted_target_is_read_bare_and_a_missing_one_names_nothing() {
    let quoted =
        Denial::of_line("touch(88) deny(1) file-write-create \"/Users/dev/My Files/x.db\"")
            .unwrap();
    assert_eq!(quoted.target, "/Users/dev/My Files/x.db");
    for line in [
        "cargo(1) deny(1) file-write-create",
        "cargo(1) deny(1) process-exec",
    ] {
        let bare = Denial::of_line(line).unwrap_or_else(|| panic!("{line}"));
        assert_eq!(bare.target, "", "{line}");
        let (block, target) = bare.block(&place());
        assert_eq!(block.kind, Kind::System, "{line}");
        assert_eq!(target, None, "{line}");
    }
}

// ---- a record, sorted -------------------------------------------------------------------------

#[test]
fn each_kind_of_denial_names_exactly_what_a_grant_would() {
    let cases = [
        (
            "docker(733) deny(1) network-outbound /private/var/run/docker.sock",
            named(
                Operation::Connect,
                Kind::LocalSocket,
                "/private/var/run/docker.sock",
            ),
        ),
        (
            "helper(43) deny(1) mach-lookup com.example.helper",
            named(Operation::Lookup, Kind::SystemService, "com.example.helper"),
        ),
        (
            "gh(4021) deny(1) mach-lookup com.apple.trustd.agent",
            named(
                Operation::Lookup,
                Kind::CertificateCheck,
                crate::sandbox::services::TRUSTD,
            ),
        ),
        (
            "cargo(31) deny(1) file-write-create /Users/dev/.cargo/registry/cache/x.crate",
            named(
                Operation::Write,
                Kind::ToolchainCache,
                "/Users/dev/.cargo/registry/cache/x.crate",
            ),
        ),
        (
            "deny network-outbound api.example.com:443 (host is not on the allow list)",
            named(Operation::Connect, Kind::Host, "api.example.com:443"),
        ),
    ];
    for (line, expected) in cases {
        assert_eq!(
            Denial::of_line(line).unwrap().block(&place()),
            expected,
            "{line}"
        );
    }
}

/// #1927: only the certificate check's own service is the certificate check. A service that
/// merely has its name in its own is a system service like any other, sorted and offered by
/// its whole name, so nothing rides the certificate check's Notice under another name.
#[test]
fn only_the_certificate_check_s_whole_name_is_sorted_as_the_certificate_check() {
    for service in [
        "com.example.trustd.helper",
        "com.apple.trustd",
        "com.apple.trustd.agent.other",
        "xcom.apple.trustd.agent",
    ] {
        let line = format!("tool(9) deny(1) mach-lookup {service}");
        assert_eq!(
            Denial::of_line(&line).unwrap().block(&place()),
            named(Operation::Lookup, Kind::SystemService, service),
            "{line}"
        );
    }
}

#[test]
fn a_denial_that_is_not_of_a_file_is_never_sorted_as_a_folder() {
    // What these name is a device class, a kernel setting, a shared-memory name: read as a
    // path it would be a file in the chat's own folder.
    for line in [
        "node(5) deny(1) iokit-open AppleKeyStoreUserClient",
        "sh(6) deny(1) sysctl-write kern.maxfiles",
        "python3(7) deny(1) ipc-posix-shm-write-create /psm_1234",
        "app(8) deny(1) mach-register com.example.listener",
    ] {
        let (block, target) = Denial::of_line(line).unwrap().block(&place());
        assert_eq!(
            (block.operation, block.kind),
            (Operation::Other, Kind::System),
            "{line}"
        );
        assert_eq!(target, None, "{line}");
    }
}

#[test]
fn listening_on_a_port_is_a_port_of_this_mac_to_listen_on_and_never_a_host() {
    // A refused bind or accept names this machine's own port (#2019): never a host to allow,
    // a port to listen on, whichever interface the server asked for.
    for (line, port) in [
        ("node(5) deny(1) network-bind *:3000", "3000"),
        ("node(5) deny(1) network-bind localhost:3000", "3000"),
        ("nc(6) deny(1) network-inbound *:8080", "8080"),
    ] {
        let (block, target) = Denial::of_line(line).unwrap().block(&place());
        assert_eq!(
            (block.operation, block.kind),
            (Operation::Listen, Kind::LocalService),
            "{line}"
        );
        assert_eq!(target.as_deref(), Some(port), "{line}");
    }
    // Another machine's address, or a port the system picks: no port to allow, a server that
    // did not start. The port the system picks is named as such (#3396), so its Notice can say
    // why; another machine's address names nothing.
    for (line, named) in [
        ("node(5) deny(1) network-bind 192.168.1.5:3000", None),
        (
            "node(5) deny(1) network-bind localhost:0",
            Some(crate::sandbox::localport::PICKED),
        ),
    ] {
        let (block, target) = Denial::of_line(line).unwrap().block(&place());
        assert_eq!(
            (block.operation, block.kind),
            (Operation::Listen, Kind::System),
            "{line}"
        );
        assert_eq!(target.as_deref(), named, "{line}");
    }
}

#[test]
fn purlis_own_denial_is_ours_and_names_nothing_to_grant() {
    let line = "purlis(48211) deny(1) file-write-create /Users/dev/plane/workspaces/alpha/x.md";
    let (block, target) = Denial::of_line(line).unwrap().block(&place());
    assert!(block.ours);
    assert_eq!(target, None);
    // A proxy names no process, so its line is nobody's own.
    let proxy = "deny network-outbound purlis.example:443 (host is not on the allow list)";
    assert!(!Denial::of_line(proxy).unwrap().block(&place()).0.ours);
}

// ---- the report a harness appended ------------------------------------------------------------

#[test]
fn the_report_is_read_past_the_harness_own_note_after_it() {
    // Claude Code's note of connections its agent proxy failed, after a failed command's report.
    let text = format!(
        "Exit code 1\nerror: something\n{}\n\n[agent-proxy] While this command ran, 1 connection \
         through the agent proxy failed:\n- api.example.com \u{2014} blocked (not allowed)\nFor \
         details: curl -sS http://localhost:1/status",
        report(&["gh(1) deny(1) mach-lookup com.example.helper"])
    );
    let read = reported(&text);
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].target, "com.example.helper");
    // Anything else after it is the command's own output, as before: a block it printed.
    let quoted = format!(
        "{}\nmore output",
        report(&["gh(1) deny(1) mach-lookup com.example.helper"])
    );
    assert!(reported(&quoted).is_empty());
    let mid = format!(
        "{}\nsee [agent-proxy] below",
        report(&["gh(1) deny(1) mach-lookup com.example.helper"])
    );
    assert!(reported(&mid).is_empty());
}

/// The harness's note is one paragraph that ends the text (review of qw355). A report quoted
/// in what a command printed, its note with it, has the rest of that output after it: that is
/// the command's own text, and names nothing.
#[test]
fn a_note_with_more_text_after_it_is_what_the_command_printed() {
    let block = report(&["gh(1) deny(1) mach-lookup com.example.helper"]);
    let note = "[agent-proxy] While this command ran, 1 connection through the agent proxy \
                failed:\n- api.example.com \u{2014} blocked (not allowed)";
    assert_eq!(reported(&format!("out\n{block}\n\n{note}\n")).len(), 1);
    for after in [
        format!("{note}\n\n$ make test\nok"),
        format!("{note}\n\n{note}"),
        format!("{note}\n \t\nthe log goes on"),
        format!("{note}\r\n\r\nthe log goes on"),
    ] {
        assert!(
            reported(&format!("out\n{block}\n\n{after}")).is_empty(),
            "{after}"
        );
    }
    // And the note is the harness's only below the report, never on its closing line.
    assert!(reported(&format!("{block} {note}")).is_empty());
}

#[test]
fn only_the_last_report_is_read_and_one_left_open_is_none() {
    let both = format!(
        "{}\nok\n{}\n",
        report(&["a(1) deny(1) mach-lookup com.example.first"]),
        report(&["b(2) deny(1) mach-lookup com.example.last"])
    );
    let read = reported(&both);
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].target, "com.example.last");
    assert!(reported("<sandbox_violations>\na(1) deny(1) mach-lookup com.example.x\n").is_empty());
    assert!(reported("no report at all").is_empty());
}

// ---- whatever the program printed or ended with -----------------------------------------------

#[test]
fn a_reported_socket_is_named_whatever_the_program_printed_or_ended_with() {
    let line = "docker(733) deny(1) network-outbound /private/var/run/docker.sock";
    let socket = named(
        Operation::Connect,
        Kind::LocalSocket,
        "/private/var/run/docker.sock",
    );
    let payloads = [
        // Down a pipe, so the line ended with another program's status, and nothing printed.
        came_back("make up 2>&1 | tail -1", "", &[line]),
        // A build runner that swallowed the client's error and said its own.
        came_back("make up", "make: *** [up] Error 1 (ignored)", &[line]),
        // A program that says the refusal in words of its own.
        failed("./run.sh", "the daemon is not available", &[line]),
        // The client's own words beside the report: one block, not two.
        failed(
            "docker ps",
            "permission denied while trying to connect to the Docker daemon socket at \
             unix:///var/run/docker.sock",
            &[line],
        ),
    ];
    for payload in payloads {
        assert_eq!(
            detect_with_targets(&payload, &place()),
            vec![socket.clone()],
            "{payload}"
        );
    }
}

#[test]
fn a_reported_service_is_named_where_the_program_blamed_something_else() {
    // A tool that reports a refused service as a failure of its own: no word of the sandbox.
    let payload = failed(
        "tool login status",
        "X Failed to log in: the token is invalid",
        &["tool(4021) deny(1) mach-lookup com.example.helper"],
    );
    assert_eq!(
        detect_with_targets(&payload, &place()),
        vec![named(
            Operation::Lookup,
            Kind::SystemService,
            "com.example.helper"
        )]
    );
}

#[test]
fn a_report_names_the_target_a_scrape_of_the_same_block_could_not() {
    // The program's words say a socket was refused and name none whole; the report names it.
    // Whichever is read first, the block is one and it keeps the name.
    let unnamed = (
        Block {
            operation: Operation::Connect,
            kind: Kind::LocalSocket,
            ours: false,
        },
        None,
    );
    let socket = named(Operation::Connect, Kind::LocalSocket, "/var/run/x.sock");
    assert_eq!(
        crate::sandboxblock::at_most(vec![unnamed.clone(), socket.clone()]),
        vec![socket.clone()]
    );
    assert_eq!(
        crate::sandboxblock::at_most(vec![socket.clone(), unnamed]),
        vec![socket.clone()]
    );
    // Two names for one block: the first stands.
    let other = named(Operation::Connect, Kind::LocalSocket, "/var/run/y.sock");
    assert_eq!(
        crate::sandboxblock::at_most(vec![socket.clone(), other]),
        vec![socket]
    );
}

#[test]
fn words_that_quote_a_report_in_what_a_command_printed_name_nothing() {
    // A log read back, a test's source: the report's words in standard output are the
    // program's, never the sandbox's.
    let quoted = report(&["docker(733) deny(1) network-outbound /var/run/docker.sock"]);
    let payload = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "cat build.log"},
        "tool_response": {"stdout": quoted, "stderr": "", "interrupted": false},
    });
    assert!(detect(&payload, &place()).is_empty());
    // And in the one string a wrapped harness hands over, a line raises a block and names
    // nothing to allow.
    let one_string = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "cat build.log"},
        "tool_response": quoted,
    });
    assert!(
        detect_with_targets(&one_string, &place())
            .iter()
            .all(|(_, target)| target.is_none())
    );
}
