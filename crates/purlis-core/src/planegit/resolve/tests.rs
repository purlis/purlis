// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::markers_in;

#[test]
fn a_conflicts_opener_closer_or_ancestor_is_a_marker_and_a_heading_rule_is_not() {
    assert!(markers_in(
        b"a\n<<<<<<< HEAD\nb\n=======\nc\n>>>>>>> mine\n"
    ));
    assert!(markers_in(b">>>>>>> mine"));
    assert!(markers_in(b"kept\n||||||| base\nold\n"));
    assert!(!markers_in(
        b"Title\n=======\n\ntext <<<<<<< inside a line\n"
    ));
}
