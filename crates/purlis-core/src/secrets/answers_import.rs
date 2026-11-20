// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The one-time import of the person's existing nevers** (M75 spec §8.10; ruling Q320; ticket
//! M75-07a, #7015, part 2).
//!
//! At the first start of this build the nevers the person already said (`app/dispatch-never.json`,
//! and the key an earlier build kept in `sandbox-grants/<clone key>.json`) are sealed into the
//! plane's store, once, and the manifest keeps the "imported once" mark. **Grants are never
//! imported.** Nevers only tighten, so carrying them over needs no press.
//!
//! # It fails closed
//!
//! - **Until it has run, the old files are authoritative and the plane is in the deny state**
//!   ([`Deny::NeversNotImported`]): a never that cannot be imported (the file does not read, a
//!   sealed never was taken away, the keyring cannot seal) is never read as "no nevers".
//! - **Idempotent**: once the manifest holds the mark, a second call changes nothing.
//! - **Nothing is imported into a plane that is in the deny state for another cause**: the person's
//!   press mends that first ([`recommit`]).
//! - The old files are left as they are, and are safe to delete once this has run.

use super::*;

/// What [`import_nevers`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Import {
    /// The old files hold no never: nothing to import, and nothing was written.
    Nothing,
    /// An earlier call imported them: nothing changed.
    Already,
    /// This process cannot seal (no keyring, or not the app): the old files stay authoritative.
    NotKept,
    /// Imported just now.
    Imported(Imported),
}

/// The nevers an import just sealed, for the window's Notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    /// Each never as `asking -> target`, in the order sealed.
    pub nevers: Vec<String>,
    /// What the Notice says.
    pub notice: String,
}

/// The text of a never in the store: `asking -> target`, as the old file's seal covers it.
pub(super) fn never_text(asking: &str, target: &str) -> String {
    format!("{asking} -> {target}")
}

/// **Whether the nevers were imported into the plane's store**: `Some(true)` once the manifest
/// holds the mark, `Some(false)` where it does not, `None` where this process cannot verify (not
/// the app, no keyring, a locked key, a manifest that does not hold its seal).
pub fn imported_nevers(root: &Path) -> Option<bool> {
    if seal::key_state() != KeyState::Usable {
        return None;
    }
    let place = plane_place(root)?;
    match read_manifest(&place) {
        Ok(Some(manifest)) => Some(manifest.imported_nevers),
        Ok(None) => Some(false),
        Err(()) => None,
    }
}

/// **Imports the person's existing nevers into the store of the plane at `root`, once** (Q320).
///
/// - `Ok(Import::Nothing)` where the old files hold none; `Ok(Import::Already)` after the first
///   import; `Ok(Import::NotKept)` where this process cannot seal, so the old files stay
///   authoritative.
/// - `Err` where a never cannot be imported (the old file does not read, a sealed never was taken
///   away, the plane is in the deny state for another cause, a write failed): **nothing was
///   changed, the old files stay authoritative and the plane stays in the deny state**
///   ([`Deny::NeversNotImported`]).
///
/// Takes the store's lock, so two starts of the app import once between them.
pub fn import_nevers(root: &Path) -> io::Result<Import> {
    let place = plane_place(root).ok_or_else(no_home)?;
    match seal::key_state() {
        KeyState::Usable => {}
        KeyState::NoKeyring | KeyState::NotTheApp => return Ok(Import::NotKept),
        KeyState::Locked => {
            return Err(io::Error::other(
                "purlis could not unlock its seal key, so it did not import your nevers: they \
                 stay as they were, and nothing is allowed until the key opens",
            ));
        }
    }
    let _turn = store_lock()?;
    let here = standing_with(root, false);
    if here.is_deny() {
        return Err(io::Error::other(
            "this project's answers are in the deny state, so purlis did not import your nevers \
             yet: accept again to mend it, and they stay as they were until then",
        ));
    }
    let found = read_place(&place, None);
    let marked = manifest_of(&found).is_some_and(|m| m.imported_nevers);
    let left = crate::dispatchnever::left_behind(root);
    // Marked, the store keeps every never the file holds, or the file is what a never said since
    // was written to and the store's write did not follow: that one is sealed now (mended).
    if marked && !file_has_more(&left, &found.entries) {
        return Ok(Import::Already);
    }
    let pairs = match left {
        crate::dispatchnever::Left::Nothing => return Ok(Import::Nothing),
        crate::dispatchnever::Left::Unread => {
            return Err(io::Error::other(
                "purlis could not read the nevers you said before, so it did not import them: \
                 they stay in their old file, which still holds, and nothing is allowed until \
                 they are imported. Restore app/dispatch-never.json as purlis last wrote it",
            ));
        }
        crate::dispatchnever::Left::Pairs(pairs) => pairs,
    };
    let id = match register_unlocked(root)? {
        Registration::Registered(id) => id,
        Registration::NotKept => return Err(cannot_seal()),
    };
    let mut draft = Draft {
        entries: here.entries,
        imported_nevers: true,
        ..Draft::default()
    };
    let mut nevers = Vec::new();
    for (asking, target) in &pairs {
        let text = never_text(asking, target);
        let kept = draft
            .entries
            .iter()
            .any(|e| e.kind == NEVER && e.text == text);
        if !kept {
            draft.entries.push(Entry::new(NEVER, &text));
        }
        if !kept || !marked {
            nevers.push(text);
        }
    }
    write(
        &place,
        Some((root, &id)),
        draft,
        found.anchor,
        manifest_of(&found).map_or(0, |m| m.counter),
    )?;
    let notice = format!(
        "purlis sealed the {} you said before: {}. They hold as they did. The old files they were \
         in are safe to delete, and doctor lists them.",
        if nevers.len() == 1 {
            "never".to_owned()
        } else {
            format!("{} nevers", nevers.len())
        },
        nevers.join(", ")
    );
    Ok(Import::Imported(Imported { nevers, notice }))
}

/// Whether the old files hold a never that `entries` (the store's) do not.
fn file_has_more(left: &crate::dispatchnever::Left, entries: &[Entry]) -> bool {
    matches!(left, crate::dispatchnever::Left::Pairs(pairs) if pairs.iter().any(|(asking, target)| {
        let text = never_text(asking, target);
        !entries.iter().any(|e| e.kind == NEVER && e.text == text)
    }))
}

/// **Whether a never in the old files is not (yet) in the store's**, for [`standing`]: the
/// nevers are not imported, or one was said in the file after the import and the store's write
/// did not follow. Either way the file is what holds, so the plane reads as the deny state until
/// [`import_nevers`] or the person's press ([`recommit`]) seals it.
pub(super) fn nevers_unsettled(root: &Path, found: &Found) -> bool {
    let marked = manifest_of(found).is_some_and(|m| m.imported_nevers);
    let left = crate::dispatchnever::left_behind(root);
    match left {
        crate::dispatchnever::Left::Nothing => false,
        crate::dispatchnever::Left::Unread => !marked,
        crate::dispatchnever::Left::Pairs(_) => !marked || file_has_more(&left, &found.entries),
    }
}

/// What a never said or lifted in the window does to the store.
pub(crate) enum Step<'a> {
    Said(&'a str, &'a str),
    Lifted(&'a str, &'a str),
}

/// **Keeps the store in step with the nevers file** (Q320): once the nevers are imported the
/// store is what the plane reads, so a never said or lifted afterwards is written there too, under
/// the store's lock. The file is written first (the caller's), so a store write that fails leaves
/// the file holding a never the store lacks, and the plane reads as the deny state
/// ([`nevers_unsettled`]) until it is mended. A lift whose store write fails leaves the never
/// holding. Answers whether the store changed.
///
/// Before the import, or where this process cannot seal, the file alone holds and nothing is done.
/// Only the person's own window actions call this ([`crate::dispatchgrant`]), never a chat.
pub(crate) fn nevers_in_step(root: &Path, step: Step<'_>) -> io::Result<bool> {
    if imported_nevers(root) != Some(true) {
        return Ok(false);
    }
    let (text, said) = match step {
        Step::Said(asking, target) => (never_text(asking, target), true),
        Step::Lifted(asking, target) => (never_text(asking, target), false),
    };
    let place = plane_place(root).ok_or_else(no_home)?;
    let _turn = store_lock()?;
    // Not counting the file: it was written just now, which is the difference this mends.
    let here = standing_with(root, false);
    if here.is_deny() {
        return Err(io::Error::other(
            "this project's answers are in the deny state, so the never was not sealed in them: \
             accept again to mend it",
        ));
    }
    let held = here
        .entries
        .iter()
        .any(|e| e.kind == NEVER && e.text == text);
    if held == said {
        return Ok(false);
    }
    let id = match register_unlocked(root)? {
        Registration::Registered(id) => id,
        Registration::NotKept => return Err(cannot_seal()),
    };
    let found = read_place(&place, None);
    let mut draft = Draft {
        entries: here.entries,
        imported_nevers: true,
        ..Draft::default()
    };
    // The person's own action in the window (see above): a never's lift widens, so it is theirs.
    // **The one place core makes a press** (#7251, minor: it belongs to the window, which would
    // pass it down through `dispatchnever` and `dispatchgrant`). It is listed, and the only one,
    // in `the_press_is_made_only_by_the_windows_command_layer`.
    #[allow(
        clippy::disallowed_methods,
        reason = "a never the person said, kept in step, until the caller hands the press down (#7251)"
    )]
    let press = Press::of_the_persons_press();
    if said {
        draft.add(&press, NEVER, &text);
    } else {
        draft.remove(&press, NEVER, &text);
    }
    write(
        &place,
        Some((root, &id)),
        draft,
        found.anchor,
        manifest_of(&found).map_or(0, |m| m.counter),
    )?;
    Ok(true)
}

/// **The files of an earlier build that the sealed store replaces** (Q320), as they exist for the
/// project at `root`: `app/sandbox.json`, `app/dispatch-never.json`, this machine's
/// `sandbox-grants/<clone key>.json` and the machine's `inbox/rules.json`, each as it exists, named
/// as the project, the config folder or the data folder names it.
pub fn old_files(root: &Path) -> Vec<String> {
    let mut named = Vec::new();
    let state = crate::names::state(root);
    for file in [
        crate::sandbox::local::legacy_path(root),
        crate::dispatchnever::path(root),
    ] {
        if file.exists() {
            named.push(
                file.strip_prefix(&state)
                    .map(|rest| rest.display().to_string())
                    .unwrap_or_else(|_| file.display().to_string()),
            );
        }
    }
    if crate::sandbox::local::path(root).exists() {
        named.push(format!(
            "{}/{}.json (in purlis's config folder)",
            crate::sandbox::local::MACHINE_GRANTS,
            crate::sandbox::local::clone_key(root)
        ));
    }
    // Looked for, never made: an existence check that needs no fence on the data home.
    let rules = crate::datahome::root_in(&crate::envvar::var)
        .map(|data| crate::askrules::Store::in_data(&data).file());
    if rules.is_some_and(|file| file.exists()) {
        named.push(format!(
            "{}/{} (in purlis's data folder)",
            crate::inboxupdates::DIR,
            crate::askrules::FILE
        ));
    }
    named
}
