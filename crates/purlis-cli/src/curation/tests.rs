// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// SK-50 (#3173): a persona's action label is the persona's own file, so a conceal, a bidi
/// override or a zero-width character in it is shown as its mark, in both listings.
#[test]
fn a_persona_action_s_label_is_shown_escaped() {
    let label = "Tidy \x1b[8mhid\u{202e}den\u{200b}";
    let shown = "Tidy ⟦U+001B⟧[8mhid⟦U+202E⟧den⟦U+200B⟧";
    assert_eq!(
        declared_line("ops", "tidy", label).as_str(),
        format!("ops/tidy  {shown}")
    );
    assert_eq!(
        offered_line("tidy", label).as_str(),
        format!("tidy  {shown}")
    );
}
