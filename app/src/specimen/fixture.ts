// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The theme specimen's sample content** (#2330): every word, line and file on the page, and
 * nothing else. The page draws these and is handed nothing of the person's (`specimen.rs`), so
 * what a chat sees of a theme is this, in it.
 *
 * Written to exercise a theme: the terminal says every ANSI colour, bright and plain, on its
 * own background; the chat shows what a harness draws (a prompt, a tool call, a diff, a
 * question); the editor holds one of each kind of token a grammar colours.
 */

const ESC = "\u001b[";
const reset = `${ESC}0m`;
const sgr = (code: number | string, text: string) => `${ESC}${code}m${text}${reset}`;

/** The ANSI names, in the order their codes run (30–37, then 90–97). */
export const ANSI_NAMES = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];

/** What the specimen's chat pane shows: a harness at work on a made-up project. */
export const CHAT_LINES: readonly string[] = [
  `${sgr(1, "> ")}Make the header stay put when the list scrolls`,
  "",
  `${sgr(36, "●")} I'll read the header first.`,
  `  ${sgr(2, "Read")} ${sgr(1, "src/Header.tsx")} ${sgr(2, "(42 lines)")}`,
  "",
  `${sgr(36, "●")} ${sgr(1, "Update")}(src/Header.tsx)`,
  `  ${sgr(2, "  12")} ${sgr("41;30", "-  position: relative;")}`,
  `  ${sgr(2, "  12")} ${sgr("42;30", "+  position: sticky;")}`,
  `  ${sgr(2, "  13")} ${sgr("42;30", "+  top: 0;")}`,
  "",
  `${sgr(32, "✓")} 14 tests passed ${sgr(2, "in 2.1 s")}`,
  `${sgr(33, "?")} Push the branch now? ${sgr(2, "(y/n)")}`,
];

/** What the specimen's terminal shows: every ANSI colour, and a test run. */
export const TERMINAL_LINES: readonly string[] = [
  `${sgr(32, "~/sample")} ${sgr(34, "main")} $ cargo test`,
  `   ${sgr("1;32", "Compiling")} sample v0.1.0`,
  `test parses_a_header ... ${sgr(32, "ok")}`,
  `test keeps_its_place ... ${sgr(31, "FAILED")}`,
  `${sgr("1;31", "error")}: 1 test failed, ${sgr(33, "1 warning")}`,
  ANSI_NAMES.map((name, n) => sgr(30 + n, name)).join(" "),
  ANSI_NAMES.map((name, n) => sgr(90 + n, name)).join(" "),
  ANSI_NAMES.map((_, n) => sgr(40 + n, "  ")).join("") +
    " " +
    ANSI_NAMES.map((_, n) => sgr(100 + n, "  ")).join(""),
];

/** The specimen's editor: a short file with a keyword, a type, a string, a number, a comment,
 *  a function and a macro, which a grammar colours each its own way. */
export const EDITOR_FILE = "sample.rs";

export const EDITOR_TEXT = `// A header that stays put while its list scrolls.
use std::time::Duration;

/// How long a scroll is let settle.
const SETTLE: Duration = Duration::from_millis(150);

pub struct Header {
    title: String,
    pinned: bool,
}

impl Header {
    pub fn new(title: &str) -> Self {
        Self { title: title.to_owned(), pinned: true }
    }
}

fn main() {
    let header = Header::new("Inbox");
    println!("{} settles in {:?}", header.title, SETTLE);
}
`;
