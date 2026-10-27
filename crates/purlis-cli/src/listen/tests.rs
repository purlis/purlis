// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn named<'a>(env: &'a [(String, String)], name: &str) -> Option<&'a str> {
    env.iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

#[test]
fn a_server_in_a_chat_that_serves_is_handed_its_socket_its_port_and_the_shim() {
    let env = env(
        5173,
        Some(Path::new("/data/serve/chat-1")),
        Some(Path::new("/tmp/chat/purlis-listen-shim.cjs")),
        Some("--max-old-space-size=4096"),
    );
    assert_eq!(named(&env, "PORT"), Some("5173"));
    assert_eq!(named(&env, "HOST"), Some("127.0.0.1"));
    assert_eq!(
        named(&env, SOCKET_ENV),
        Some("/data/serve/chat-1/5173.sock")
    );
    assert_eq!(named(&env, PORT_ENV), Some("5173"));
    assert_eq!(
        named(&env, "NODE_OPTIONS"),
        Some("--require \"/tmp/chat/purlis-listen-shim.cjs\" --max-old-space-size=4096")
    );
}

#[test]
fn a_server_in_a_chat_that_may_not_serve_yet_is_handed_its_port_alone() {
    let env = env(5173, None, None, Some("--inspect"));
    assert_eq!(named(&env, "PORT"), Some("5173"));
    assert_eq!(named(&env, SOCKET_ENV), None);
    assert_eq!(
        named(&env, "NODE_OPTIONS"),
        None,
        "Node's own options stay as they were"
    );
}

#[test]
fn a_shim_path_with_a_quote_is_quoted_for_node() {
    let env = env(
        3000,
        Some(Path::new("/d")),
        Some(Path::new("/tmp/a \"b\"/s.cjs")),
        None,
    );
    assert_eq!(
        named(&env, "NODE_OPTIONS"),
        Some("--require \"/tmp/a \\\"b\\\"/s.cjs\"")
    );
}

#[test]
fn the_shim_moves_only_the_chats_own_port_onto_its_socket() {
    assert!(SHIM.contains("process.env.PURLIS_SERVE_SOCKET"));
    assert!(SHIM.contains("process.env.PURLIS_SERVE_PORT"));
    assert!(SHIM.contains("if (wanted(args) !== port) return listen.apply(this, args);"));
}

#[test]
fn each_run_writes_its_shim_new_under_a_name_of_its_own() {
    let dir = tempfile::tempdir().expect("a folder");
    let first = shim_in(dir.path()).expect("written");
    let second = shim_in(dir.path()).expect("written");
    assert_ne!(first, second);
    assert_eq!(std::fs::read_to_string(&first).expect("read"), SHIM);
}
