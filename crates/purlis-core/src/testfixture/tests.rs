// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_fixture_name_loses_its_suffix_and_any_other_name_is_kept() {
    assert_eq!(real_name(OsStr::new(".claude.fixture")), ".claude");
    assert_eq!(
        real_name(OsStr::new("opencode.json.fixture")),
        "opencode.json"
    );
    assert_eq!(real_name(OsStr::new("charter.toml")), "charter.toml");
    assert_eq!(real_name(OsStr::new(".fixture")), ".fixture");
    assert_eq!(
        real_path(Path::new("a/.claude.fixture/settings.json")),
        PathBuf::from("a/.claude/settings.json")
    );
}
