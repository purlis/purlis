// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::{Os, fill, line, on_pool, on_this_os};

#[test]
fn computer_reads_mac_on_macos() {
    assert_eq!(
        on_this_os(line::W21_HERE, Os::MacOs),
        "Anthropic's certificate isn't trusted on this Mac. purlis uses this Mac's proxy settings \
         and trusted certificates."
    );
    assert_eq!(on_this_os(line::W21_HERE, Os::Linux), line::W21_HERE);
    assert_eq!(on_this_os(line::W21_HERE, Os::Windows), line::W21_HERE);
}

#[test]
fn your_keychain_reads_your_system_keychain_off_macos() {
    assert_eq!(
        on_this_os(line::W10, Os::Linux),
        "Your system keychain is locked. Unlock it to start this chat."
    );
    assert_eq!(
        on_this_os(line::W10, Os::Windows),
        on_this_os(line::W10, Os::Linux)
    );
    assert_eq!(on_this_os(line::W10, Os::MacOs), line::W10);
}

#[test]
fn a_runner_line_names_its_pool_after_the_thing_it_names() {
    assert_eq!(
        fill(
            line::W109,
            &[
                ("file", &on_pool("policy.toml", Some("gpu"))),
                ("version", "0.6.0")
            ]
        ),
        "purlis: policy.toml on gpu requires purlis 0.6.0 or later, so this request was blocked."
    );
    assert_eq!(
        fill(line::W26A, &[("lock", &on_pool("The model lock", None))]),
        "The model lock now blocks this chat's requests."
    );
}

#[test]
fn a_slot_is_filled_once_and_its_value_is_never_read_as_a_slot() {
    assert_eq!(
        fill(line::W25, &[("reason", "it named <reason>")]),
        "purlis blocked this request: it named <reason>."
    );
}
