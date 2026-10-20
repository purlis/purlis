// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a runner chat asks to write in the project, in the person's words** (#2046, ADR 0078
//! §7, RR-17): persona, workspace and memory changes made on a runner arrive on the desktop as
//! proposals, which only the person applies. A brokered write ([`crate::brokered::Write`]) from a
//! chat on a pool's runner is held, and shown as [`said`] says it, never written until the person
//! applies it.

use crate::brokered::{Store, Write};

/// The longest stretch of a write's text shown in its proposal's line: the rest is said to be
/// there, never cut silently.
pub const MOST_SHOWN_CHARS: usize = 600;

fn store(of: Store) -> &'static str {
    match of {
        Store::Workspace => "this workspace's memory",
        Store::Persona => "its persona's memory",
        Store::Shared => "shared memory",
    }
}

fn shown(text: &str) -> String {
    let text = text.trim();
    let count = text.chars().count();
    if count <= MOST_SHOWN_CHARS {
        return text.to_owned();
    }
    let kept: String = text.chars().take(MOST_SHOWN_CHARS).collect();
    format!("{kept}… ({} more characters)", count - MOST_SHOWN_CHARS)
}

fn titled(title: Option<&str>, text: &str) -> String {
    match title {
        Some(title) => format!("“{}”: {}", title.trim(), shown(text)),
        None => shown(text),
    }
}

/// **What `write` would do**, said in one sentence the person reads before they apply it: the
/// store or file it writes, and the text it carries, held to [`MOST_SHOWN_CHARS`].
pub fn said(write: &Write) -> String {
    match write {
        Write::PersonaRemember {
            text,
            title,
            shared,
        } => format!(
            "Remember in {}: {}",
            if *shared {
                "shared memory"
            } else {
                "its persona's memory"
            },
            titled(title.as_deref(), text)
        ),
        Write::WorkspaceRemember { text, title } => format!(
            "Remember in this workspace's memory: {}",
            titled(title.as_deref(), text)
        ),
        Write::Todo { text } => format!("Add a todo to this workspace: {}", shown(text)),
        Write::WorkspaceVision { text } => {
            format!("Replace this workspace's vision with: {}", shown(text))
        }
        Write::WorkspaceSection { section, text } => format!(
            "Add to this workspace's {}: {}",
            section.header(),
            shown(text)
        ),
        Write::Forget { of, slug } => format!("Forget memory {slug} in {}", store(*of)),
        Write::EditMemory {
            of,
            slug,
            title,
            text,
        } => match (title, text) {
            (_, Some(text)) => format!(
                "Rewrite memory {slug} in {}: {}",
                store(*of),
                titled(title.as_deref(), text)
            ),
            (Some(title), None) => format!(
                "Retitle memory {slug} in {} as “{}”",
                store(*of),
                title.trim()
            ),
            (None, None) => format!("Rewrite memory {slug} in {}, as it is", store(*of)),
        },
        Write::ArchiveMemory { of, slug } => format!("Archive memory {slug} in {}", store(*of)),
        Write::UnarchiveMemory {
            of,
            slug,
            restore_as,
        } => match restore_as {
            Some(name) => format!(
                "Bring memory {slug} back from the archive of {} as {name}",
                store(*of)
            ),
            None => format!(
                "Bring memory {slug} back from the archive of {}",
                store(*of)
            ),
        },
        Write::MoveMemory { from, slug, to } => {
            format!("Move memory {slug} from {} to {}", store(*from), store(*to))
        }
        Write::WorkspaceOptimize { stale_days } => format!(
            "Tidy this workspace's memory, letting go of what is older than {stale_days} days"
        ),
        Write::PersonaOptimize {
            of,
            with_shared,
            stale_days,
        } => format!(
            "Tidy {}{}, letting go of what is older than {stale_days} days",
            store(*of),
            if *with_shared {
                " and shared memory"
            } else {
                ""
            }
        ),
        Write::TodoClose { how, slug } => format!("Close todo {slug} ({})", how.verb()),
        Write::WorkspaceCreate { name, vision } => match vision {
            Some(vision) => format!("Make workspace {name}, its vision: {}", shown(vision)),
            None => format!("Make workspace {name}"),
        },
    }
}

#[cfg(test)]
mod tests;
