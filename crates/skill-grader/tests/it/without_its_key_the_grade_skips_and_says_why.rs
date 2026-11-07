// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Without the `skill-grades` environment's key (not set up yet, or a fork's pull request,
//! which GitHub never hands a secret), the grade starts no harness and says so in one line.

use crate::fixture::fixture;
use skill_grader::{Graded, grade};

#[test]
fn no_key_starts_no_harness_and_says_the_grade_was_skipped() {
    let fixture = fixture(true, true);
    for key in [None, Some(""), Some("  ")] {
        let Graded::Skipped(line) = grade(&fixture.ask(key)) else {
            panic!("it graded without a key");
        };
        assert!(line.contains("skipped"), "{line}");
        assert!(line.contains("`skill-grades`"), "{line}");
        assert!(line.contains("ANTHROPIC_API_KEY"), "{line}");
        assert!(line.contains("nothing was graded"), "{line}");
    }
    assert!(!fixture.argv.exists(), "a harness was started");
}
