// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis policy check` with no path (T1a-64): this machine's file, judged as the reader judges
//! it. A policy fixture stands in for `/etc/purlis/policy.json` (R-699).

use super::check;
use super::{Fixture, Locks, Of, Seen, set_for_this_test, use_fixture_for_this_test};

fn installed(fixture: Fixture) -> check::Checked {
    use_fixture_for_this_test(fixture);
    let checked = check::installed();
    set_for_this_test(Locks::none());
    checked
}

#[test]
fn policy_check_judges_the_installed_file_as_the_reader_does() {
    let dir = tempfile::tempdir().expect("a folder");
    let file = dir.path().join("policy.json");
    std::fs::write(&file, r#"{"sandbox": {"opt-out": false}}"#).expect("written");
    let judged = format!("file: {} (judged as purlis reads it)", file.display());

    let trusted = installed(Fixture::at(&file));
    assert_eq!(
        trusted.lines,
        [
            judged.as_str(),
            "lowest purlis version that reads this file: 0.5.0",
            "chats on a key: not refused",
            "chats on a sign-in: not refused",
        ]
    );
    assert_eq!(trusted.status, 0);

    // The same file, owned by someone other than root: refused as the reader refuses it.
    let not_root = installed(Fixture::at(&file).seen_as(|seen: Seen, of: Of| match of {
        Of::File => Seen { uid: 501, ..seen },
        Of::Folder => seen,
    }));
    assert_eq!(
        not_root.lines,
        [
            judged.as_str(),
            "not read: it is not owned by the system's administrator",
        ]
    );
    assert_eq!(not_root.status, 2);
}
