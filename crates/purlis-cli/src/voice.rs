// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! How charter talks: `charter/util.py`'s four loggers and `charter/tui.py`'s column
//! arithmetic, byte for byte.
//!
//! Every line a person or an agent reads from a ported command goes through here, because
//! the words ARE the interface: an agent that reads `charter recall` at session start
//! parses what it prints, and a harness that runs both charters must not be able to tell
//! them apart. The glyph is coloured only when stderr is a terminal — the message never is.
//!
//! A message is [`Markup`] (SK-54, #3225): a literal is purlis's own words, and a `String` —
//! any `format!` that splices a name, a reason or a path — enters escaped, whole. A line that
//! wants purlis's colour builds it with [`Markup::styled`].

use std::io::IsTerminal;
use std::path::Path;

use purlis_core::tui::{Markup, Sgr};

/// One line in purlis's voice: the mark, in its colour when `coloured`, then the message.
fn said(colour: Sgr, mark: &'static str, msg: Markup, coloured: bool) -> Markup {
    let mark = Markup::new().own(mark);
    let glyph = if coloured {
        Markup::new().styled(colour, mark)
    } else {
        mark
    };
    glyph.own(" ").then(msg)
}

fn say(colour: Sgr, mark: &'static str, msg: Markup) {
    eprintln!(
        "{}",
        said(colour, mark, msg, std::io::stderr().is_terminal())
    );
}

/// `util.info`: `• <msg>` on stderr.
pub fn info(msg: impl Into<Markup>) {
    say(Sgr::Cyan, "•", msg.into());
}

/// `util.ok`: `✓ <msg>` on stderr.
pub fn ok(msg: impl Into<Markup>) {
    say(Sgr::Green, "✓", msg.into());
}

/// `util.warn`: `! <msg>` on stderr.
pub fn warn(msg: impl Into<Markup>) {
    say(Sgr::Yellow, "!", msg.into());
}

/// `util.err`: `✗ <msg>` on stderr.
pub fn err(msg: impl Into<Markup>) {
    say(Sgr::Red, "✗", msg.into());
}

/// `str.rstrip()`, with Python's idea of whitespace.
///
/// In the core, because `charter status` trims its own table rows there and two spellings of
/// "what counts as trailing whitespace" is how one row keeps a space the other drops.
pub use purlis_core::memstore::py_rstrip;

/// `tui.column("", cells)`: the widest cell, plus the two-space gap to the next column.
///
/// **Measured in COLUMNS, by [`purlis_core::tui`], and no longer in characters.** This
/// module used to carry its own `sanitize`/`width`/`pad` — a hand-rolled second copy of
/// `charter/tui.py` that counted one column per character, with the precondition written
/// beside it that "charter's own labels are ASCII". Half the cells here are not charter's: a
/// recall hit's label is a memory's heading, which is whatever an operator wrote. A CJK
/// heading declares two columns per glyph and a combining mark declares none, so the count
/// under-padded one and over-padded the other and the row stopped lining up with its
/// neighbours — against a Python that has measured east-asian width since it was written.
///
/// So there is one width in this repo now, and this is a call into it.
pub fn column<'a>(cells: impl IntoIterator<Item = &'a str>) -> usize {
    purlis_core::tui::column("", cells, 2, None)
}

/// `tui.pad(text, w)` for a column [`column`] sized: escaped, then fitted to `w`.
///
/// `text` is a cell of untrusted text, and the padding is where it is made safe to show: every
/// control, bidi, zero-width and tag character comes out as `⟦U+XXXX⟧`, measured as shown
/// ([`purlis_core::tui::pad`], SK-47). A caller does not escape a cell first.
pub fn pad(text: &str, w: usize) -> String {
    purlis_core::tui::pad(text, w, purlis_core::tui::Align::Left)
}

/// A path as charter prints it: relative to the plane where it is inside, else as is.
pub fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// `workspace.say_unread`: one sentence on stderr for each path charter could not look at.
///
/// The sentence itself is [`purlis_core::memstore::cannot_check`], because `docs generate`
/// says the same one about a persona's memory store from inside the core, and one unreadable
/// path worded two ways is two different repairs for one directory.
pub fn unread(root: &Path, unread: &purlis_core::memstore::Unread) {
    for (path, code) in unread {
        err(purlis_core::memstore::cannot_check(root, path, *code).to_string());
    }
}

/// `charter/workspace.py`'s remedy clause, from the core for the same reason: the commands
/// that run there and the commands that print here must not be able to word one unreadable
/// path two ways.
pub use purlis_core::memstore::uncheckable_fix;

#[cfg(test)]
mod tests;
