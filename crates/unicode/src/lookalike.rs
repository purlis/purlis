// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Punctuation a reader takes for something it is not, or reads past** (#6218, #6601): what
//! the core's reading of a marked link's words (`purlis_core::panelinks`'s OSC 8 kind) asks of a
//! character beyond UTS #39's skeleton.
//!
//! These two lists are kept by hand, and are the only ones in this crate that are: no Unicode
//! property names them. UTS #39's confusables map most look-alikes of `.` to `.`, and the ones
//! it maps elsewhere are [`DOTS`]; what a sentence puts around a word is a reader's habit, not a
//! general category ([`WRAPPERS`]). A character added to either changes which marked links ask
//! first, so each change is a change to the core's table of asks, read there.

/// Characters a reader may take for a dot that UTS #39's skeleton maps to something else.
pub const DOTS: &str =
    "\u{0589}\u{3002}\u{FF61}\u{06D4}\u{00B7}\u{2027}\u{02F3}\u{0387}\u{30FB}\u{FF65}";

/// The punctuation a sentence puts around a word, which a reader reads past.
pub const WRAPPERS: &str = "()[]{}<>\"'`*_,;:!?.«»“”‘’「」";

/// Whether a reader may take `c` for a dot where UTS #39's skeleton does not ([`DOTS`]).
pub fn reads_as_a_dot(c: char) -> bool {
    DOTS.contains(c)
}

/// Whether `c` is punctuation a sentence puts around a word ([`WRAPPERS`]).
pub fn wraps_a_word(c: char) -> bool {
    WRAPPERS.contains(c)
}
