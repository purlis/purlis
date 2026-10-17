// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// macOS only: off macOS `op_team_id` is `None` by construction, and there is no `codesign`.
#[cfg(target_os = "macos")]
#[test]
fn a_binary_signed_by_no_team_or_not_signed_at_all_has_no_team() {
    // Apple's own binaries are signed with `TeamIdentifier=not set`.
    assert_eq!(op_team_id(Path::new("/usr/bin/true")), None);
    let bin = tempfile::tempdir().unwrap();
    let op = stand_in::program(bin.path(), "op", "#!/bin/sh\n");
    assert_eq!(op_team_id(&op), None);
}
