// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use purlis_core::skills::source::policy::{Act, Audited};

use super::*;

fn an_install() -> Audited {
    Audited {
        act: Act::Install,
        level: "project".into(),
        skills: vec!["notify".into()],
        source: "https://github.com/acme/skills".into(),
        pin: "0123456789abcdef0123456789abcdef01234567".into(),
    }
}

#[test]
fn inside_a_chat_the_audit_is_refused_before_any_host_is_looked_for() {
    for mark in [
        purlis_core::hookwire::CHAT_ENV,
        purlis_core::hookwire::SOCKET_ENV,
        purlis_core::hookwire::SANDBOXED_ENV,
        purlis_core::active::SESSION_ID_ENV,
    ] {
        let env = |name: &str| (name == mark).then(|| "1".to_owned());

        let refused = audit_in(&env, Path::new("/no/such/project"), &an_install()).expect_err(mark);

        assert_eq!(refused, IN_A_CHAT, "{mark}");
    }
}
