// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_column_is_its_widest_cell_and_a_gap() {
    assert_eq!(column(["2026-03-02", "—"]), 12);
    assert_eq!(pad("—", 12), "—           ");
    // A cell is text: what would reorder or colour it is shown, and counted as shown.
    assert_eq!(column(["a\u{202e}b"]), 12);
    assert_eq!(pad("a\u{202e}b", 12), "a⟦U+202E⟧b  ");
    assert_eq!(pad("\x1b[31mx", 13), "⟦U+001B⟧[31mx");
}
