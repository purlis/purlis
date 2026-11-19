// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `answers` rows (M75 slice 07a, #7015, part 2; spec §8.10): what doctor says of the one
//! sealed answer store ([`crate::secrets::answers`]).
//!
//! - **`answers seal`** (Linux and Windows): the declared seal gap, in the research's words. A
//!   declared fact, not a fault: the row is green and the chat still starts.
//! - **`answers device id`**: whether `context.machine` is set. Only the app reads the registry,
//!   so a doctor run elsewhere says it did not check, never "unset".
//! - **`answers store`**: which state the project's answers are in, and why where denied.
//! - **`answers old files`**: the files an earlier build kept, safe to delete once their nevers
//!   are imported (Q320).
//! - **`answers moved plane`**: the declared gap: a project moved to another filesystem with no
//!   link left is not yet seen (slice 11's).
//!
//! Rows are only printed by a doctor run on the person's own machine, never by one a test names.

use super::{Doctor, Row};
use crate::sandbox::Os;
use crate::secrets::answers::{self, Deny, Device, State};

const SEAL: &str = "answers seal";
const DEVICE: &str = "answers device id";
const STORE: &str = "answers store";
const OLD_FILES: &str = "answers old files";
const MOVED: &str = "answers moved plane";

/// **The declared seal gap** (Q309 point 4, Q319 C6, Q320a N1 and N4), as the research words it.
/// `None` on macOS, where the Keychain holds the seal key to the app.
pub(super) fn seal_gap_row(os: Os) -> Option<Row> {
    let detail = match os {
        Os::Linux => {
            "declared gap: on Linux any program you run can read the key purlis seals your \
             answers with, so the seal stops a hand edit and a sandboxed chat only. Asking for a \
             password each time purlis starts narrows the gap and does not close it."
        }
        Os::Windows => {
            "declared gap: on Windows any program you run can read the key purlis seals your \
             answers with, and purlis has no Windows sandbox yet, so the seal stops hand edits \
             only. Confirming with Windows Hello each time purlis starts narrows the gap and does \
             not close it."
        }
        Os::MacOs | Os::Other => return None,
    };
    Some(Row::ok(SEAL, detail))
}

/// What the registry says of the device id, as doctor can tell.
pub(super) enum DeviceView {
    /// This process cannot read the registry (it is not the app).
    NotChecked,
    /// What the registry holds.
    Seen(Device),
}

/// **The device id row**: `context.machine` is set only once the person confirmed the sealed id
/// in the window; with no keyring, or a registry that does not read, it is unset and said so.
pub(super) fn device_row(view: &DeviceView) -> Row {
    match view {
        DeviceView::NotChecked => Row::ok(
            DEVICE,
            "not checked here: only purlis's app can read the sealed device id, so Settings in the \
             window says whether it is confirmed",
        ),
        DeviceView::Seen(Device::Confirmed(_)) => {
            Row::ok(DEVICE, "confirmed: a rule can name this machine")
        }
        DeviceView::Seen(Device::Unconfirmed(_)) => Row::warn(
            DEVICE,
            "sealed and not confirmed yet, so no rule can name this machine",
            "Confirm the device id in the window, under Settings. Until then a rule that names the \
             machine does not match.",
        ),
        DeviceView::Seen(Device::None) => Row::warn(
            DEVICE,
            answers::DEVICE_UNSET,
            "Open a project in purlis's window with a keyring that works, and confirm the id there.",
        ),
    }
}

/// What one cause of the deny state says, in words.
pub(super) fn cause_said(cause: &Deny) -> String {
    cause.said()
}

/// **The store row**, from a state.
pub(super) fn store_row(state: &State) -> Row {
    match state {
        State::Fresh => Row::ok(STORE, "nothing sealed yet: a fresh install"),
        State::Holds => Row::ok(STORE, "sealed, and every answer holds its seal"),
        State::NoKeyring => Row::warn(
            STORE,
            "this machine has no keyring purlis can use, so it cannot seal your answers and the \
             shipped default applies",
            "Nothing you answer is kept until a keyring works.",
        ),
        State::Unverifiable => Row::ok(
            STORE,
            "not checked here: only purlis's app can verify the sealed answers, so the window \
             says what state they are in",
        ),
        State::Deny(causes) => Row::fail(
            STORE,
            format!(
                "in the deny state, so nothing the answers allow holds: {}",
                causes.iter().map(cause_said).collect::<Vec<_>>().join("; ")
            ),
            "Open the project in purlis's window and accept again to mend it.",
        ),
    }
}

/// How much doctor knows of the import.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Imported {
    /// The manifest holds the mark.
    Yes,
    /// It does not.
    No,
    /// This process cannot say (not the app, no keyring).
    Unknown,
}

/// **The old files row**: the files an earlier build kept that the sealed store replaces (Q320).
/// Nothing is said where none is there. Where the nevers are imported they are safe to delete;
/// where they are not, deleting them would lose a never, so the row says so instead.
pub(super) fn old_files_row(files: &[String], imported: Imported) -> Option<Row> {
    if files.is_empty() {
        return None;
    }
    let mut listed = files.to_vec();
    listed.push("inbox/rules.json (in purlis's data folder)".to_owned());
    let named = listed.join(", ");
    Some(match imported {
        Imported::Yes => Row::ok(
            OLD_FILES,
            format!(
                "safe to delete: {named}. Their nevers are sealed in the answer store, and \
                 grants are not carried over. Until the dispatch family's migration removes its \
                 old reader, deleting app/dispatch-never.json makes purlis refuse a dispatch \
                 between personas until you say never again."
            ),
        ),
        Imported::No => Row::warn(
            OLD_FILES,
            format!("{named}: the nevers in them are not imported yet"),
            "Keep them. Purlis's app imports the nevers once, at its first start of this build; \
             open the project in the window.",
        ),
        Imported::Unknown => Row::ok(
            OLD_FILES,
            format!(
                "{named}: safe to delete once purlis's app has imported the nevers in them, which \
                 only the app can say"
            ),
        ),
    })
}

/// **The declared gap of a moved project**: slice 11's to close.
pub(super) fn moved_row() -> Row {
    Row::ok(
        MOVED,
        "declared gap: a project moved to another filesystem with no link left at its old path is \
         not yet seen, since only a move on one filesystem keeps the .git identity the registry \
         checks. A move purlis makes itself carries the answers. Closing this is slice 11's \
         (holding the project folder against a rename).",
    )
}

/// The rows for the project `d` reads.
pub(super) fn answers(d: &Doctor) -> Vec<Row> {
    if !d.answers_here || !d.has_plane {
        return Vec::new();
    }
    let mut rows = Vec::new();
    rows.extend(seal_gap_row(Os::this()));
    let standing = answers::standing(&d.root);
    let view = if standing.state == State::Unverifiable {
        DeviceView::NotChecked
    } else {
        DeviceView::Seen(answers::device())
    };
    rows.push(device_row(&view));
    rows.push(store_row(&standing.state));
    let imported = match answers::imported_nevers(&d.root) {
        Some(true) => Imported::Yes,
        Some(false) => Imported::No,
        None => Imported::Unknown,
    };
    rows.extend(old_files_row(&answers::old_files(&d.root), imported));
    rows.push(moved_row());
    rows
}

#[cfg(test)]
#[path = "answers_tests.rs"]
mod tests;
