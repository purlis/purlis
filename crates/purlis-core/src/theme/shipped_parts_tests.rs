// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every shipped appearance's parts are measured in one look** (#4410): as written, nothing
//! tinted. A launch measures the default and Purlis Classic's dark before its deadline starts, and
//! every other shipped appearance inside the wait, which a measure in every tint does not fit. A
//! see-through colour on a token their parts use, or a part that reaches a consent family in a
//! tint, fails here instead of drawing the window without its parts at every launch.

use super::super::{built_ins, purlis};
use super::measured_afresh;

#[test]
fn every_shipped_appearance_s_parts_are_measured_as_written_alone() {
    let shipped: Vec<_> = purlis().appearances.iter().chain(built_ins()).collect();
    assert_eq!(shipped.len(), 5, "Purlis's three and Classic's two");
    for one in shipped {
        assert_eq!(
            measured_afresh(one),
            (1, 0),
            "{} {:?} (high contrast: {}) is measured in more than its own look",
            one.name,
            one.appearance,
            one.high_contrast,
        );
    }
}
