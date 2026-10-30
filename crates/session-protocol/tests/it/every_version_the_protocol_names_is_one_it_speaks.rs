// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every version the session protocol names is one it speaks, said once** (#2207).
//!
//! A `KNOWS_*` version gates what a host sends a link, so one past [`VERSION`] would gate it off
//! every link there is. And [`VERSION`]'s own doc says what each minor added, once: a minor two
//! changes both took, or one a change took without saying, fails here. Minors that were passed
//! over when their changes landed under later numbers are listed, so no new gap goes unseen.

use purlis_session_protocol::session::VERSION;

const SOURCE: &str = include_str!("../../src/session.rs");

/// Minors no change added: their numbers were passed over. 47 was held for #3677, which landed
/// after 1.48 as 1.49: a build of 1.48 says it knows every frame up to 1.48, so 47 stays unfilled.
const PASSED_OVER: &[u16] = &[12, 19, 20, 47, 54];

/// Each `pub const KNOWS_…: Version` in the source, with its minor.
fn knows() -> Vec<(String, u16)> {
    let mut found = Vec::new();
    let mut rest = SOURCE;
    while let Some(at) = rest.find("pub const KNOWS_") {
        rest = &rest[at + "pub const ".len()..];
        let name = rest[..rest.find(':').expect("a constant's type")].to_owned();
        let body = &rest[..rest.find("};").expect("a version's end")];
        assert!(body.contains("major: 1,"), "{name} is of major 1: {body}");
        let minor = body
            .split("minor:")
            .nth(1)
            .and_then(|after| after.trim().trim_end_matches(',').parse().ok())
            .unwrap_or_else(|| panic!("{name} names its minor: {body}"));
        found.push((name, minor));
    }
    found
}

/// The minors [`VERSION`]'s doc says were added, in its order: each paragraph that opens
/// `/// 1.<minor> `.
fn told() -> Vec<u16> {
    let doc = &SOURCE[..SOURCE
        .find("pub const VERSION: Version")
        .expect("the version")];
    doc.lines()
        .filter_map(|line| line.strip_prefix("/// 1."))
        .filter_map(|rest| rest.split(' ').next()?.parse().ok())
        .collect()
}

#[test]
fn no_version_a_host_gates_on_is_past_the_one_it_speaks() {
    let knows = knows();
    assert!(!knows.is_empty(), "the source names its KNOWS_ versions");
    for (name, minor) in knows {
        assert!(
            minor <= VERSION.minor,
            "{name} is 1.{minor}, past the protocol's 1.{}",
            VERSION.minor
        );
        assert!(
            told().contains(&minor),
            "{name} is 1.{minor}, which the version's doc does not say was added"
        );
    }
}

#[test]
fn the_versions_doc_says_each_minor_once_in_order_up_to_the_one_it_speaks() {
    let told = told();
    let every: Vec<u16> = (1..=VERSION.minor)
        .filter(|minor| !PASSED_OVER.contains(minor))
        .collect();
    assert_eq!(
        told, every,
        "the doc of `VERSION` names each minor up to 1.{} once, in order",
        VERSION.minor
    );
}
