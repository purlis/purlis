// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Network refusals wherever a command that may have met one said them (#1663, #1972): on
//! standard output as well as standard error, and with the host a lookup was of; never in what
//! a line that only reads or prints handed back, nor in a background command's output read back.

use super::*;
use crate::sandbox::Os;
use serde_json::json;

const ROOT: &str = "/Users/dev/plane";
const CHAT: &str = "/Users/dev/plane/workspaces/alpha/repo";

fn place() -> Place<'static> {
    Place {
        root: Path::new(ROOT),
        chat: Path::new(CHAT),
        cwd: Path::new(CHAT),
        home: Some(Path::new("/Users/dev")),
    }
}

fn failed(command: &str, error: &str) -> serde_json::Value {
    json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "error": error,
    })
}

fn came_back(command: &str, stdout: &str, stderr: &str) -> serde_json::Value {
    json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "tool_response": {"stdout": stdout, "stderr": stderr, "interrupted": false},
    })
}

fn lookup(host: Option<&str>) -> (Block, Option<String>) {
    (
        Block {
            operation: Operation::Lookup,
            kind: Kind::Host,
            ours: false,
        },
        host.map(str::to_owned),
    )
}

/// Codex's `PostToolUse`: both streams in one string, with nothing of how the command ended.
fn one_string(command: &str, said: &str) -> serde_json::Value {
    json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "tool_response": said,
    })
}

/// The live case: a dispatched task ran a database client through a brokered run with both
/// streams on standard output. Go's resolver said "no such host" there, and no Notice came.
#[test]
fn a_lookup_a_2_1_run_said_on_standard_output_is_a_block_naming_its_host() {
    let command =
        "purlis secret exec prod-db -- sh -c 'usql \"$DSN\" -c \"select 1\"' 2>&1 | tail -5";
    let said = "error: dial tcp: lookup db.prod.example.com: no such host\n";
    // The command failed: Claude Code hands its output over as the failure's error.
    assert_eq!(
        detect_with_targets(&failed(command, &format!("Exit code 1\n{said}")), &place()),
        vec![lookup(Some("db.prod.example.com"))]
    );
    // Codex says nothing of how it ended either: read by its line the same way.
    assert_eq!(
        detect_with_targets(&one_string(command, said), &place()),
        vec![lookup(Some("db.prod.example.com"))]
    );
    // Go's resolver naming the server it asked says the same.
    assert_eq!(
        detect_with_targets(
            &one_string(
                command,
                "lookup db.prod.example.com on 192.168.1.1:53: no such host"
            ),
            &place()
        ),
        vec![lookup(Some("db.prod.example.com"))]
    );
    // It came back (the pipe's `tail` succeeded): its line runs a program, which may have met
    // the refusal it said.
    assert_eq!(
        detect_with_targets(&came_back(command, said, ""), &place()),
        vec![lookup(Some("db.prod.example.com"))]
    );
}

/// A background command's output read back later, with `cat`, with the harness's file tool or
/// with its own tool for it, says nothing of how that command ended: it raises no block
/// (#1972). The sandbox's own report of the command, where the harness keeps one, still does.
#[test]
fn a_background_commands_output_read_back_is_no_block() {
    let output = "Unable to find image 'postgres:16' locally\n\
                  docker: permission denied while trying to connect to the Docker daemon socket \
                  at unix:///Users/dev/.docker/run/docker.sock: Head \
                  \"http://%2FUsers%2Fdev%2F.docker%2Frun%2Fdocker.sock/_ping\": dial unix \
                  /Users/dev/.docker/run/docker.sock: connect: operation not permitted.\n\
                  error: dial tcp: lookup db.prod.example.com: no such host\n";
    let output_file = "/private/tmp/claude-501/-Users-dev-plane/0b1c/tasks/b7x2.output";
    let read = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Read",
        "tool_input": {"file_path": output_file},
        "tool_response": {"type": "text", "file": {"filePath": output_file, "content": output}},
    });
    let polled = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "BashOutput",
        "tool_input": {"bash_id": "b7x2"},
        "tool_response": {"stdout": output, "stderr": "", "status": "completed"},
    });
    for payload in [
        came_back("cat /private/tmp/tasks/b7x2.output", output, ""),
        read,
        polled,
    ] {
        assert_eq!(
            detect_with_targets(&payload, &place()),
            Vec::new(),
            "{payload}"
        );
    }
}

/// Each way a program says a lookup was refused, with the host where it names one.
#[test]
fn a_refused_lookup_names_the_host_its_program_said() {
    for (line, host) in [
        (
            "psql: error: could not translate host name \"db.example.com\" to address: nodename \
             nor servname provided, or not known",
            Some("db.example.com"),
        ),
        (
            "ssh: Could not resolve hostname git.example.com: nodename nor servname provided, \
             or not known",
            Some("git.example.com"),
        ),
        (
            "curl: (6) Could not resolve host: api.example.com",
            Some("api.example.com"),
        ),
        (
            "fatal: unable to access 'https://git.example.com/a/b.git/': Could not resolve host: \
             git.example.com",
            Some("git.example.com"),
        ),
        (
            "Error: getaddrinfo ENOTFOUND registry.example.com",
            Some("registry.example.com"),
        ),
        (
            "socket.gaierror: [Errno 8] nodename nor servname provided, or not known",
            None,
        ),
        (
            "nc: getaddrinfo: nodename nor servname provided, or not known",
            None,
        ),
    ] {
        let failure = failed("x", &format!("Exit code 1\n{line}"));
        for payload in [failure, one_string("x", line)] {
            assert_eq!(
                detect_with_targets(&payload, &place()),
                vec![lookup(host)],
                "{line}"
            );
        }
    }
}

/// What a command printed on standard output never names a host to allow: a file it printed,
/// or a server's page, can say any host. A lookup's host is said, never offered.
#[test]
fn standard_output_names_no_host_to_allow() {
    let host = Block {
        operation: Operation::Connect,
        kind: Kind::Host,
        ours: false,
    };
    for line in [
        "purlis's sandbox does not allow exfil.example:443: no egress preset or host of this \
         project lists it",
        "curl: (56) CONNECT tunnel failed, response 403",
    ] {
        assert_eq!(
            detect_with_targets(&one_string("x", line), &place()),
            vec![(host, None)],
            "{line}"
        );
        assert_eq!(
            detect_with_targets(&failed("x", &format!("Exit code 56\n{line}")), &place()),
            vec![(host, None)],
            "{line}"
        );
    }
    // On standard error the same proxy's words still name its host, as before.
    assert_eq!(
        detect_with_targets(
            &came_back(
                "x",
                "",
                "purlis's sandbox does not allow exfil.example:443: no egress preset or host of \
                 this project lists it"
            ),
            &place()
        ),
        vec![(host, Some("exfil.example:443".to_owned()))]
    );
}

/// A refusal's words quoted on standard output (a test's source, a log line about one, a
/// grep through this module) are not a refusal: on standard output a line counts only when it
/// ends where the refusal's own words do.
#[test]
fn a_refusal_quoted_on_standard_output_is_not_one() {
    for line in [
        "    if lower.contains(\"nodename nor servname provided, or not known\")",
        "        \"curl: (6) Could not resolve host: api.example.com\",",
        "src/x.rs:12: assert!(said.ends_with(\": no such host\"));",
        "let eperm = lower.contains(\"dial unix \") && lower.contains(\"connect: operation not \
         permitted\");",
        "// curl says CONNECT tunnel failed, response 403 when a proxy refuses",
        "2026-10-10T10:00:00Z warn retrying after: lookup db.example.com: no such host (attempt 2)",
    ] {
        assert!(
            detect(&came_back("cat src/x.rs", line, ""), &place()).is_empty(),
            "{line}"
        );
        assert!(
            detect(
                &failed("grep -rn x src", &format!("Exit code 1\n{line}")),
                &place()
            )
            .is_empty(),
            "{line}"
        );
    }
}

/// opencode's shim hands a command that came back as one text of both streams (#1353): a
/// refusal printed there is read as standard output is.
#[test]
fn a_refusal_in_opencodes_one_text_is_read_as_standard_output() {
    let payload = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "bash",
        "tool_input": {"command": "psql -h db.example.com"},
        "tool_response": "connecting\nerror: dial tcp: lookup db.example.com: no such host\n",
    });
    assert_eq!(
        detect_with_targets(&payload, &place()),
        vec![lookup(Some("db.example.com"))]
    );
    // A failed command's mixed standard error names a lookup's host too, and still no host
    // to allow.
    let mixed = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "bash",
        "tool_response": {"stderr": "curl: (6) Could not resolve host: api.example.com\n\
                                     purlis's sandbox does not allow x.example:443: no egress \
                                     preset or host of this project lists it",
                          "mixed": true},
    });
    assert_eq!(
        detect_with_targets(&mixed, &place()),
        vec![
            lookup(Some("api.example.com")),
            (
                Block {
                    operation: Operation::Connect,
                    kind: Kind::Host,
                    ours: false
                },
                None
            )
        ]
    );
}

/// A word that is no host's is never said as one.
#[test]
fn a_lookup_names_only_a_word_that_could_be_a_host() {
    for line in [
        "curl: (6) Could not resolve host: ",
        "lookup $(rm -rf /): no such host",
        "Error: getaddrinfo ENOTFOUND <script>",
    ] {
        let found = detect_with_targets(&came_back("x", "", line), &place());
        assert!(
            found.iter().all(|(_, target)| target.is_none()),
            "{line}: {found:?}"
        );
    }
}

// ---- the host travels on the Block, and is kept with it ----------------------------------------

/// Each Block `blocks` names, kept in a network record under a fresh data home as the app keeps
/// it (`record::Entry::blocked`), and every line of it as written.
fn kept_rows(blocks: &[(Block, Option<&str>, u64)]) -> (Vec<serde_json::Value>, String) {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("project");
    let record = record::Record::in_data(&dir.path().join("data"));
    for (block, target, at) in blocks {
        let entry = record::Entry::blocked(block, *target, record::Chat::default(), None, *at);
        record.write(&root, &entry).expect("kept");
    }
    let text = std::fs::read_to_string(record.file(&root)).expect("kept");
    let rows = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("json"))
        .collect();
    (rows, text)
}

/// The host of a refused connection is kept with its Block as a grant would name it, and the
/// host of a refused lookup apart from it, as one a program named: shown, never offered to
/// allow. A path, and a host that is no host, is never kept.
#[test]
fn the_host_travels_on_the_block_and_is_kept_with_it() {
    let connect = Block {
        operation: Operation::Connect,
        kind: Kind::Host,
        ours: false,
    };
    let looked_up = lookup(None).0;
    let write = Block {
        operation: Operation::Write,
        kind: Kind::Home,
        ours: false,
    };
    let (rows, text) = kept_rows(&[
        (connect, Some("API.Example.com:443"), 100),
        (looked_up, Some("db.example.com"), 101),
        (connect, Some("localhost:5432"), 102),
        (write, Some("/Users/dev/secret/notes.txt"), 103),
        (connect, None, 104),
        (looked_up, Some("169.254.169.254"), 105),
    ]);
    assert_eq!(rows[0]["target"], "api.example.com:443");
    assert!(rows[0].get("looked_up").is_none());
    assert_eq!(rows[1]["looked_up"], "db.example.com");
    assert!(
        rows[1].get("target").is_none(),
        "a lookup's host is never one to allow"
    );
    for row in &rows[2..] {
        assert!(
            row.get("target").is_none() && row.get("looked_up").is_none(),
            "{row}"
        );
    }
    assert!(!text.contains("secret"));
    // Counted as ever.
    let entries: Vec<record::Entry> = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("entry"))
        .collect();
    let connects = record::counts(&entries, 200)
        .into_iter()
        .find(|count| count.operation == Operation::Connect)
        .expect("counted");
    assert_eq!(connects.blocks, 3);
    // The doctor's top refused hosts are refused connections' alone.
    assert_eq!(
        record::hosts_refused(&entries, 200),
        vec![("api.example.com:443".to_owned(), 1)]
    );
}

/// A line kept with no host, as a build from before #1663 wrote one, still reads and counts.
#[test]
fn a_line_kept_without_a_host_still_counts() {
    let line = r#"{"at":100,"event":"block","block":{"operation":"lookup","kind":"host","ours":false},"outcome":"refused"}"#;
    let older: record::Entry = serde_json::from_str(line).expect("an older line reads");
    assert_eq!(older.looked_up, None);
    let (rows, text) = kept_rows(&[(lookup(None).0, Some("db.example.com"), 150)]);
    assert_eq!(rows.len(), 1);
    let mut entries: Vec<record::Entry> = text
        .lines()
        .map(|one| serde_json::from_str(one).expect("entry"))
        .collect();
    entries.insert(0, older);
    assert_eq!(
        record::counts(&entries, 200)
            .iter()
            .map(|c| c.blocks)
            .sum::<u64>(),
        2
    );
}

// ---- a chat behind purlis's own proxy -----------------------------------------------------------

/// A harness purlis wraps (Codex, opencode) reaches the network through purlis's own proxy,
/// which tells the app each host it refused by name. Its hook leaves those to it, so one
/// refusal is one Notice; a lookup or a socket it still reads.
#[test]
fn a_chat_behind_purlis_proxy_leaves_its_hosts_to_the_proxy() {
    assert!(the_proxy_tells_hosts(Some("codex")));
    assert!(the_proxy_tells_hosts(Some("opencode")));
    assert!(!the_proxy_tells_hosts(Some("claude")));
    assert!(!the_proxy_tells_hosts(None));
    assert!(!the_proxy_tells_hosts(Some("something-else")));

    let mixed = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "bash",
        "tool_response": {"stderr": "curl: (6) Could not resolve host: api.example.com\n\
                                     purlis's sandbox does not allow x.example:443: no egress \
                                     preset or host of this project lists it",
                          "mixed": true},
    });
    let found = detect_with_targets(&mixed, &place());
    assert_eq!(found.len(), 2);
    assert_eq!(
        the_hooks_own(found.clone(), Some("opencode"), Os::MacOs),
        vec![lookup(Some("api.example.com"))],
        "the proxy tells the host"
    );
    assert_eq!(
        the_hooks_own(found.clone(), Some("claude"), Os::MacOs),
        found
    );
}

/// On Linux a chat purlis wraps has every file, socket and service its sandbox refused told by
/// the wrap's supervisor, by its exact name (#2344): its hook leaves those to it too, so one
/// refusal is one Notice. A lookup past the proxy, which no supervisor sees, it still reads.
#[test]
fn a_chat_purlis_wraps_on_linux_leaves_its_files_sockets_and_services_to_the_supervisor() {
    let file = |operation| {
        (
            Block {
                operation,
                kind: Kind::Home,
                ours: false,
            },
            None,
        )
    };
    let found = vec![
        lookup(Some("api.example.com")),
        (
            Block {
                operation: Operation::Connect,
                kind: Kind::LocalSocket,
                ours: false,
            },
            None,
        ),
        local_service(Some("5432")),
        file(Operation::Write),
        file(Operation::Read),
        file(Operation::File),
    ];
    for harness in ["codex", "opencode"] {
        assert_eq!(
            the_hooks_own(found.clone(), Some(harness), Os::Linux),
            vec![lookup(Some("api.example.com"))],
            "{harness}"
        );
    }
    // Claude Code runs its own sandbox on Linux, which no supervisor of purlis's tells.
    assert_eq!(
        the_hooks_own(found.clone(), Some("claude"), Os::Linux),
        found
    );
}

/// A service on this Mac refused (qw384): a connection to one loopback port.
fn local_service(port: Option<&str>) -> (Block, Option<String>) {
    (
        Block {
            operation: Operation::Connect,
            kind: Kind::LocalService,
            ours: false,
        },
        port.map(str::to_owned),
    )
}

/// qw384, the live case: kubectl against a local cluster at 127.0.0.1:50795 failed with Go's
/// `EPERM`, and no Notice came. The failed command's own error names the port.
#[test]
fn a_go_client_refused_a_port_on_this_mac_is_a_block_naming_the_port() {
    let said = "E1014 13:04:11 memcache.go:265] couldn't get current server API group list: Get \
                \"https://127.0.0.1:50795/api?timeout=32s\": dial tcp 127.0.0.1:50795: connect: \
                operation not permitted\nUnable to connect to the server: dial tcp \
                127.0.0.1:50795: connect: operation not permitted";
    assert_eq!(
        detect_with_targets(
            &failed("kubectl get pods", &format!("Exit code 1\n{said}")),
            &place()
        ),
        vec![local_service(Some("50795"))]
    );
}

/// qw384: the sandbox's own report of the refusal names the port too, whatever the program said.
#[test]
fn the_sandboxs_own_line_for_a_port_on_this_mac_names_the_port() {
    let stderr = "error: could not connect\n<sandbox_violations>\n\
                  kubectl(812) deny(1) network-outbound 127.0.0.1:50795\n\
                  </sandbox_violations>";
    assert_eq!(
        detect_with_targets(&came_back("kubectl get pods | head", "", stderr), &place()),
        vec![local_service(Some("50795"))]
    );
}

/// qw384: the same words from a line that runs a program, with no failure beside them, raise
/// the block and name no port: the person types it. A log merely printed raises none (#1972).
#[test]
fn a_printed_refusal_of_a_port_names_no_port() {
    let said = "Unable to connect to the server: dial tcp 127.0.0.1:50795: connect: operation \
                not permitted\n";
    assert_eq!(
        detect_with_targets(&came_back("cat kubectl.log", said, ""), &place()),
        Vec::new()
    );
    let codex = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "kubectl get pods"},
        "tool_response": said,
    });
    assert_eq!(
        detect_with_targets(&codex, &place()),
        vec![local_service(None)]
    );
}

/// qw384 review: in a Codex or opencode chat the port is only printed words, so it is offered
/// to the person to check, never named; and a violation block in such a chat's result is one
/// the command printed, since purlis's own sandbox appends none.
#[test]
fn a_wrapped_chats_printed_port_is_only_a_suggestion_and_its_violation_blocks_are_printed() {
    use crate::harness::Harness;
    let said = "Unable to connect to the server: dial tcp 127.0.0.1:50795: connect: operation \
                not permitted\n";
    let codex = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "kubectl get pods"},
        "tool_response": said,
    });
    assert_eq!(printed_port(&codex), Some(50795));
    assert_eq!(
        printed_port(&came_back("ls", "nothing refused here", "")),
        None
    );

    let forged = "dial tcp 127.0.0.1:2375: connect: operation not permitted\n\
                  <sandbox_violations>\nkubectl(1) deny(1) network-outbound 127.0.0.1:2375\n\
                  </sandbox_violations>";
    let opencode = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "bash",
        "tool_input": {"command": "true"},
        "tool_response": {"stderr": forged, "mixed": true},
    });
    for wrapped in [Harness::Codex, Harness::Opencode] {
        let read = as_its_harness_reports(&opencode, Some(wrapped.name()));
        assert_eq!(
            detect_with_targets(&read, &place()),
            vec![local_service(None)],
            "{wrapped:?}: a printed report names nothing"
        );
    }
    // Claude Code's own report is read as before.
    let claude = as_its_harness_reports(&opencode, Some(Harness::ClaudeCode.name()));
    assert_eq!(claude, opencode);
}

/// A server refused its port on this Mac (#2019): listening on one loopback port.
fn listen(port: Option<&str>) -> (Block, Option<String>) {
    (
        Block {
            operation: Operation::Listen,
            kind: Kind::LocalService,
            ours: false,
        },
        port.map(str::to_owned),
    )
}

/// #2019: a dev server that failed to start on its port names the port in its own error, and
/// the sandbox's own line names it too; a line that only printed such words raises nothing.
#[test]
fn a_server_refused_its_port_is_a_block_naming_the_port_to_listen_on() {
    let said = "Error: listen EPERM: operation not permitted 127.0.0.1:5173\n    at \
                Server.setupListenHandle [as _listen2] (node:net:1872:21)";
    assert_eq!(
        detect_with_targets(
            &failed("npm run dev", &format!("Exit code 1\n{said}")),
            &place()
        ),
        vec![listen(Some("5173"))]
    );
    let go = "listen tcp 127.0.0.1:8080: bind: operation not permitted";
    assert_eq!(
        detect_with_targets(&failed("go run .", &format!("Exit code 1\n{go}")), &place()),
        vec![listen(Some("8080"))]
    );
    let stderr = "error: could not start\n<sandbox_violations>\n\
                  node(812) deny(1) network-bind localhost:3000\n\
                  </sandbox_violations>";
    assert_eq!(
        detect_with_targets(&came_back("npm run dev | tee log", "", stderr), &place()),
        vec![listen(Some("3000"))]
    );
    assert_eq!(
        detect_with_targets(&came_back("cat server.log", said, ""), &place()),
        Vec::new()
    );
    // In a Codex chat the port is printed words: offered to the person to check, never named.
    let codex = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "npm run dev"},
        "tool_response": said,
    });
    assert_eq!(detect_with_targets(&codex, &place()), vec![listen(None)]);
    assert_eq!(printed_port(&codex), Some(5173));
}

/// #2019: on Linux the wrap's supervisor tells a refused listen itself, so the hook leaves it.
#[test]
fn a_chat_purlis_wraps_on_linux_leaves_a_refused_listen_to_the_supervisor() {
    for harness in ["codex", "opencode"] {
        assert_eq!(
            the_hooks_own(vec![listen(Some("3000"))], Some(harness), Os::Linux),
            Vec::new(),
            "{harness}"
        );
    }
}
