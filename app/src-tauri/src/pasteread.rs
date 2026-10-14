// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The clipboard's text, read for a Paste the person pressed** in a terminal's menu.
//!
//! A terminal's keyboard paste is the engine handing the terminal a `paste` event with the
//! text in it. A menu's row gets no such event, and the page may not read the clipboard itself,
//! so the core reads it and answers the text to the window, which hands it to the terminal's
//! own paste: the path the keyboard's paste takes, bracketed when the program asked.
//!
//! What holds this to that one use:
//!
//! - **It is the window's alone** (`session-protocol`'s `WINDOW_ONLY`), and of the
//!   value-bearing class (`ipc_commands.rs`, ADR 0052): what is on the clipboard may be a
//!   vault's value. No link is served it, so no chat, tool or hook reaches it.
//! - **It takes no argument.** Nothing the caller says decides where the text goes: it goes
//!   back to the window that asked, and nowhere else.
//! - **It reads when it is asked and at no other time.** Nothing polls the clipboard, nothing
//!   keeps what was read, and nothing of it is written down: not the text, not its length.
//! - **Whatever text the clipboard holds, a vault's copied value too.** The keyboard's paste
//!   hands the same value to the same terminal, and Paste pastes. The read goes by a handle of
//!   its own, opened for it and dropped after: it does not touch what a vault's Copy keeps to
//!   clear the clipboard by, and waits on nothing a vault's clear holds.
//! - **A read that fails says it failed**, and is not answered as a clipboard holding nothing.
//! - **Text only.** A clipboard holding a picture or files answers as one holding nothing.
//! - **Bounded** ([`PASTE_LIMIT`]): more than that is refused, in a sentence the pane says.

/// The most text a menu's Paste hands a terminal: 1 MiB. A paste is typed into a program a
/// byte at a time, and past this it is somebody's file, not something to type.
pub(crate) const PASTE_LIMIT: usize = 1024 * 1024;

/// What a clipboard holding more than [`PASTE_LIMIT`] is answered with.
pub(crate) const TOO_MUCH: &str =
    "The clipboard holds more than 1 MiB of text, which is more than purlis pastes from a menu.";

/// What the window is answered, given what the clipboard said it holds: its text; nothing,
/// where it holds no text or an empty one; or a refusal where it holds too much.
pub(crate) fn pasted(held: Option<String>) -> Result<Option<String>, String> {
    match held {
        Some(text) if text.len() > PASTE_LIMIT => Err(TOO_MUCH.to_owned()),
        Some(text) if !text.is_empty() => Ok(Some(text)),
        _ => Ok(None),
    }
}

/// What a read of the clipboard that failed is answered with, before the system's own words.
pub(crate) const NOT_READ: &str = "purlis could not read the clipboard";

/// What the system's answer to "your text" means: the text; none where the clipboard holds
/// something that is not text, or nothing; or, for any other failure, that the read failed and
/// what the system said, which is never what the clipboard holds.
pub(crate) fn held(read: Result<String, arboard::Error>) -> Result<Option<String>, String> {
    match read {
        Ok(text) => Ok(Some(text)),
        Err(arboard::Error::ContentNotAvailable) => Ok(None),
        Err(failed) => Err(format!("{NOT_READ}: {failed}")),
    }
}

/// The system clipboard's text now, read once by a handle opened for the read and dropped
/// after it: nothing of it outlives the answer.
fn read() -> Result<Option<String>, String> {
    let mut board = arboard::Clipboard::new().map_err(|failed| format!("{NOT_READ}: {failed}"))?;
    held(board.get_text())
}

/// The clipboard's text, for the window to paste into the terminal the person pressed Paste
/// on ([`pasted`]). None where the clipboard holds no text. Read off the window's thread: a
/// clipboard another program serves can take its time.
#[tauri::command]
#[specta::specta]
pub(crate) async fn clipboard_text() -> Result<Option<String>, String> {
    let read = tauri::async_runtime::spawn_blocking(read)
        .await
        .map_err(|_| format!("{NOT_READ}."))?;
    pasted(read?)
}

#[cfg(test)]
#[path = "pasteread_tests.rs"]
mod tests;
