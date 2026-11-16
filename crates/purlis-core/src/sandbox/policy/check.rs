// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis policy check` (stub).

use std::path::Path;

/// What the check says, one line per fact, and its exit status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    pub lines: Vec<String>,
    pub status: u8,
}

/// This machine's policy file, judged as the reader judges it.
pub fn installed() -> Checked {
    Checked { lines: Vec::new(), status: 0 }
}

/// A draft at `path`.
pub fn draft(_path: &Path) -> Checked {
    Checked { lines: Vec::new(), status: 0 }
}
