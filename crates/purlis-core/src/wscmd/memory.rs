// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A workspace's memory verbs: `workspace forget`, `edit`, `archive`, `unarchive` and
//! `move-memory` (and `persona move-memory`, which is the same move), with what each says.
//!
//! **Two entrances, one operation** (ADR 0067 §2): the terminal's `purlis` calls these with its
//! printer, and the app calls them for a sandboxed chat that asked over its hook socket
//! ([`crate::brokered`]), collecting the lines to hand back. Either way the command prints the
//! same lines. Each answers its exit code and whether a committed store changed; the caller
//! says [`next_save`] when one did.

use std::path::Path;

use crate::memscope::Scope;
use crate::memstore;
use crate::repocmd::{Say, Sink};
use crate::workspaces::Plane;

/// What purlis says once a memory changed, where the project's `[plane] mode` saves it
/// (`commit_memory_reactive`): nothing commits a memory on its own, it travels with the
/// project's next save (ADR 0051). `None` for a project that names no mode, or `off`.
pub fn next_save(root: &Path) -> Option<String> {
    use crate::planesave::Mode;
    let mode = crate::planesave::Settings::read(root).plane.mode.value;
    match mode {
        Some(mode @ (Mode::Commit | Mode::Push | Mode::Pr | Mode::PrMerge)) => Some(format!(
            "This plane's [plane] mode is {}: the memory goes with the plane's next save — \
             `purlis save`.",
            mode.as_str()
        )),
        _ => None,
    }
}

/// [`next_save`], said.
pub fn say_next_save(root: &Path, say: Sink) {
    if let Some(line) = next_save(root) {
        say(Say::Info(line.into()));
    }
}

/// What purlis says once a memory is written to workspace `name`'s journal at `path`: where it
/// went, and whether it stays on this disk or is shared.
pub fn said_remembered(plane: &Plane, name: &str, path: &Path, no_sync: bool, say: Sink) {
    say(Say::Done(
        format!(
            "Remembered in '{name}' → workspaces/{name}/memory/{}",
            path.file_name().unwrap_or_default().to_string_lossy()
        )
        .into(),
    ));
    if !plane.is_live(name) {
        say(Say::Info(
            format!(
                "  '{name}' is LOCAL (private) — memory stays on disk, not committed. Make it \
             shareable: purlis workspace live {name}"
            )
            .into(),
        ));
    } else if no_sync {
        say(Say::Info(
            "  (--no-sync) recorded locally; share it later with: purlis save"
                .to_owned()
                .into(),
        ));
    } else {
        say_next_save(plane.root(), say);
    }
}

/// The sentence a workspace memory operation that did nothing says: the one `workspace forget`
/// says for the same refusals.
fn refused(name: &str, slug: &str, e: &std::io::Error, say: Sink) -> (u8, bool) {
    match e.kind() {
        std::io::ErrorKind::NotFound => say(Say::Fail(
            format!(
                "no memory '{}' in workspace '{name}' (list them: purlis workspace recall).",
                crate::shown::short(slug)
            )
            .into(),
        )),
        std::io::ErrorKind::InvalidInput
            if !crate::contain::segment_ok(slug.strip_suffix(".md").unwrap_or(slug)) =>
        {
            say(Say::Fail(
                format!(
                    "'{}' is not the slug of one memory — name a file in \
                 workspaces/{name}/memory/, not a path.",
                    crate::shown::short(slug)
                )
                .into(),
            ))
        }
        _ => say(Say::Fail(e.to_string().into())),
    }
    (1, false)
}

/// `workspace forget`: delete one journal entry by slug or filename, and its index line.
pub fn forget(plane: &Plane, name: &str, slug: &str, say: Sink) -> Result<(u8, bool), String> {
    let ws = plane.workspace(name).map_err(|e| e.to_string())?;
    Ok(
        match memstore::forget(plane.root(), &ws.dir().join("memory"), slug) {
            Ok(()) => {
                say(Say::Done(
                    format!("Forgot '{slug}' from workspace '{name}'.").into(),
                ));
                (0, true)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                say(Say::Fail(
                    format!(
                        "no memory '{slug}' in workspace '{name}' (list them: purlis workspace \
                     recall)."
                    )
                    .into(),
                ));
                (1, false)
            }
            // A slug is one file in the store. `../<elsewhere>` is a path, and the store's own
            // resolver is never handed one: this is where M1.1's round 3 found a slug deleting
            // a file outside the plane. A slug the ends of two entries' names share, or the
            // index, is the core's sentence.
            Err(e) => refused(name, slug, &e, say),
        },
    )
}

/// `workspace edit`: rewrite one journal entry in place (ADR 0065): the filename and the stamp
/// kept, the index line retitled, and whatever was not given kept as the file has it.
pub fn edit(
    plane: &Plane,
    name: &str,
    slug: &str,
    title: Option<&str>,
    text: Option<&str>,
    say: Sink,
) -> Result<(u8, bool), String> {
    let ws = plane.workspace(name).map_err(|e| e.to_string())?;
    let exact = match ws.typed_memory(slug, memstore::Typed::Stored) {
        Ok(exact) => exact,
        Err(e) => return Ok(refused(name, slug, &e, say)),
    };
    let opened = match ws.open_memory(&exact) {
        Ok(opened) => opened,
        Err(e) => return Ok(refused(name, slug, &e, say)),
    };
    let (title, text) = opened.revised(title, text);
    Ok(
        match ws.edit_memory(&exact, &title, &text, memstore::Base::Read(&opened.text)) {
            Ok(path) => {
                say(Say::Done(
                    format!(
                        "Edited '{slug}' in workspace '{name}' → {}",
                        crate::personaverbs::rel(plane.root(), &path)
                    )
                    .into(),
                ));
                (0, true)
            }
            Err(memstore::EditRefused::Io(e)) => refused(name, slug, &e, say),
            Err(stale) => {
                say(Say::Fail(stale.to_string().into()));
                (1, false)
            }
        },
    )
}

/// `workspace archive`: move one journal entry into `memory/archive/` and drop its index line:
/// the window's Delete, and reversible (ADR 0065).
pub fn archive(plane: &Plane, name: &str, slug: &str, say: Sink) -> Result<(u8, bool), String> {
    let ws = plane.workspace(name).map_err(|e| e.to_string())?;
    let exact = match ws.typed_memory(slug, memstore::Typed::Archiving) {
        Ok(exact) => exact,
        Err(e) => return Ok(refused(name, slug, &e, say)),
    };
    Ok(match ws.archive_memory(&exact) {
        Ok(path) => {
            say(Say::Done(
                format!(
                    "Archived '{slug}' from workspace '{name}' → {}. Undo: `purlis workspace \
                 unarchive {} -w {name}`",
                    crate::personaverbs::rel(plane.root(), &path),
                    memstore::undo_args(&exact, &path)
                )
                .into(),
            ));
            (0, true)
        }
        Err(e) => refused(name, slug, &e, say),
    })
}

/// `workspace unarchive`: move an archived journal entry back and re-index it: the window's
/// Undo (ADR 0065).
pub fn unarchive(
    plane: &Plane,
    name: &str,
    slug: &str,
    restore_as: Option<&str>,
    say: Sink,
) -> Result<(u8, bool), String> {
    let ws = plane.workspace(name).map_err(|e| e.to_string())?;
    let exact = match ws.typed_memory(slug, memstore::Typed::Restoring) {
        Ok(exact) => exact,
        Err(e) => return Ok(refused(name, slug, &e, say)),
    };
    Ok(match ws.unarchive_memory(&exact, restore_as) {
        Ok(path) => {
            say(Say::Done(
                format!(
                    "Restored '{slug}' to workspace '{name}' → {}",
                    crate::personaverbs::rel(plane.root(), &path)
                )
                .into(),
            ));
            (0, true)
        }
        Err(e) => refused(name, slug, &e, say),
    })
}

/// The flag that sends a memory to `scope`.
fn to_flag(scope: &Scope) -> String {
    match scope {
        Scope::Workspace(name) => format!("--to-workspace {name}"),
        Scope::Persona(name) => format!("--to-persona {name}"),
        Scope::Shared => "--to-shared".to_owned(),
    }
}

/// The command that moves the memory now named `stem` in `at` back to `home`, when there is
/// one to print: a memory in shared memory is named through a persona, `persona` (the one the
/// command was given) or the project's default.
fn way_back(
    plane: &Plane,
    at: &Scope,
    stem: &str,
    home: &Scope,
    persona: Option<&str>,
) -> Option<String> {
    let back = to_flag(home);
    match at {
        Scope::Workspace(name) => Some(format!(
            "purlis workspace move-memory {stem} -w {name} {back}"
        )),
        Scope::Persona(name) => Some(format!("purlis persona move-memory {name} {stem} {back}")),
        Scope::Shared => {
            let persona = persona
                .map(str::to_owned)
                .or_else(|| plane.default_persona())?;
            Some(format!(
                "purlis persona move-memory {persona} {stem} --shared {back}"
            ))
        }
    }
}

/// `workspace move-memory` and `persona move-memory`: move one memory from `from` to `to`
/// (KN-3), renamed whole, its title and stamp kept, its index line moved with it. A slug typed
/// is made exact as `edit` makes it. `persona` is the one the command was given, which names
/// the way back from shared memory.
pub fn move_memory(
    plane: &Plane,
    from: &Scope,
    slug: &str,
    to: &Scope,
    persona: Option<&str>,
    now: chrono::NaiveDateTime,
    say: Sink,
) -> (u8, bool) {
    if !crate::contain::segment_ok(slug.strip_suffix(".md").unwrap_or(slug)) {
        say(Say::Fail(
            format!(
                "'{}' is not the slug of one memory — name a file in {}/, not a path.",
                crate::shown::short(slug),
                from.store()
            )
            .into(),
        ));
        return (1, false);
    }
    let refused = |e: &std::io::Error, say: Sink| {
        if e.kind() == std::io::ErrorKind::NotFound && e.to_string().starts_with("no such memory") {
            say(Say::Fail(
                format!(
                    "no memory '{}' in {}.",
                    crate::shown::short(slug),
                    from.said()
                )
                .into(),
            ));
        } else {
            say(Say::Fail(e.to_string().into()));
        }
        (1, false)
    };
    let exact = match crate::memscope::typed(plane, from, slug) {
        Ok(exact) => exact,
        Err(e) => return refused(&e, say),
    };
    match crate::memscope::move_memory(plane, from, &exact, to, now) {
        Ok(path) => {
            let stem = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let mut said = format!(
                "Moved '{slug}' from {} to {} → {}",
                from.said(),
                to.said(),
                crate::personaverbs::rel(plane.root(), &path)
            );
            if let Some(back) = way_back(plane, to, &stem, from, persona) {
                // Not an Undo to the letter: a journal name comes back to the minute, as the
                // stamp line the memory carries holds minutes.
                said.push_str(&format!(". To move it back: `{back}`"));
            }
            say(Say::Done(said.into()));
            (0, true)
        }
        Err(e) => refused(&e, say),
    }
}
