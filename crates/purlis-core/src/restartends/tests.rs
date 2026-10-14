// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// A chat's harness (100) with an MCP server of its own (101), a shell its command ran in (102),
/// a dev server under that shell (103, listening), a build under it (104) and a program of
/// another chat (200).
const LISTING: &str = "  100     1 /usr/local/bin/claude
  101   100 /usr/local/bin/node
  102   100 /bin/zsh
  103   102 /usr/local/bin/node
  104   102 cargo
  105   104 rustc
  200     1 /bin/zsh
  201   200 /usr/bin/python3
";

const LSOF: &str = "p103\nf21\nn*:5173\nf22\nn[::1]:5173\np201\nf3\nn*:8000\n";

#[test]
fn what_a_restart_ends_is_what_the_chat_started_and_what_of_it_listens() {
    let ends = ends_of(&below(LISTING, 100), &listening_in(LSOF));
    assert_eq!(
        ends,
        Ends {
            listening: vec![Listening {
                program: "node".to_owned(),
                port: 5173,
            }],
            // cargo and rustc: run through the chat's shell. Its own MCP server is the
            // harness's, and starts again with it.
            others: 2,
        }
    );
}

#[test]
fn another_chat_s_programs_are_never_counted() {
    let ends = ends_of(&below(LISTING, 100), &listening_in(LSOF));
    assert!(!ends.listening.iter().any(|one| one.port == 8000));
}

#[test]
fn a_chat_that_started_nothing_ends_nothing() {
    let ends = ends_of(&below("  100     1 claude\n  101   100 node\n", 100), &[]);
    assert_eq!(ends, Ends::default());
}

#[test]
fn a_port_listened_on_twice_by_one_program_is_said_once_and_ports_come_in_order() {
    let lsof = "p103\nn*:9229\nn127.0.0.1:5173\nn[::1]:5173\n";
    let ends = ends_of(&below(LISTING, 100), &listening_in(lsof));
    assert_eq!(
        ends.listening
            .iter()
            .map(|one| one.port)
            .collect::<Vec<_>>(),
        [5173, 9229]
    );
}

#[test]
fn a_line_that_is_not_a_process_is_passed_over() {
    let listed = below(
        "garbage\n  100 1 claude\n  x 100 node\n  102 100 /bin/bash\n",
        100,
    );
    assert_eq!(
        listed,
        [Below {
            pid: 102,
            parent: 100,
            program: "bash".to_owned()
        }]
    );
}
