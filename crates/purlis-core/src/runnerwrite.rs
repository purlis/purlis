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

/// `text`, held to `most` characters where there is a most: the rest is said to be there.
fn shown(text: &str, most: Option<usize>) -> String {
    let count = text.chars().count();
    let Some(most) = most.filter(|most| count > *most) else {
        return text.to_owned();
    };
    let kept: String = text.chars().take(most).collect();
    let more = count - most;
    format!(
        "{kept}… ({more} more {})",
        crate::counts::plural(more, "character", "characters")
    )
}

fn titled(title: Option<&str>, text: &str, most: Option<usize>) -> String {
    match title {
        Some(title) => format!("“{title}”: {}", shown(text, most)),
        None => shown(text, most),
    }
}

/// **What `write` would do**, said in one sentence the person reads before they apply it: the
/// store or file it writes, and the text it carries, held to [`MOST_SHOWN_CHARS`].
pub fn said(write: &Write) -> String {
    said_held_to(write, Some(MOST_SHOWN_CHARS))
}

/// [`said`], with every text it carries whole: what a held write's own tab shows.
pub fn said_whole(write: &Write) -> String {
    said_held_to(write, None)
}

/// Whether [`said`] cuts a text `write` carries.
pub fn cut(write: &Write) -> bool {
    texts(write)
        .iter()
        .any(|text| text.chars().count() > MOST_SHOWN_CHARS)
}

/// **`write` as it is held**: each text it carries trimmed, as it is shown, so Apply writes
/// exactly what the person read.
pub fn held_as(write: Write) -> Write {
    let trim = |text: String| text.trim().to_owned();
    let trim_some = |text: Option<String>| text.map(trim);
    match write {
        Write::PersonaRemember {
            text,
            title,
            shared,
        } => Write::PersonaRemember {
            text: trim(text),
            title: trim_some(title),
            shared,
        },
        Write::WorkspaceRemember { text, title } => Write::WorkspaceRemember {
            text: trim(text),
            title: trim_some(title),
        },
        Write::Todo { text } => Write::Todo { text: trim(text) },
        Write::WorkspaceVision { text } => Write::WorkspaceVision { text: trim(text) },
        Write::WorkspaceSection { section, text } => Write::WorkspaceSection {
            section,
            text: trim(text),
        },
        Write::EditMemory {
            of,
            slug,
            title,
            text,
        } => Write::EditMemory {
            of,
            slug,
            title: trim_some(title),
            text: trim_some(text),
        },
        Write::WorkspaceCreate { name, vision } => Write::WorkspaceCreate {
            name,
            vision: trim_some(vision),
        },
        Write::WorkspaceSnapshot { description, force } => Write::WorkspaceSnapshot {
            description: trim_some(description),
            force,
        },
        other => other,
    }
}

/// Every text `write` carries that the person reads: its texts, titles and names.
pub fn texts(write: &Write) -> Vec<&str> {
    match write {
        Write::PersonaRemember { text, title, .. } | Write::WorkspaceRemember { text, title } => {
            [Some(text.as_str()), title.as_deref()]
                .into_iter()
                .flatten()
                .collect()
        }
        Write::Todo { text }
        | Write::WorkspaceVision { text }
        | Write::WorkspaceSection { text, .. } => vec![text.as_str()],
        Write::EditMemory {
            slug, title, text, ..
        } => [Some(slug.as_str()), title.as_deref(), text.as_deref()]
            .into_iter()
            .flatten()
            .collect(),
        Write::Forget { slug, .. }
        | Write::ArchiveMemory { slug, .. }
        | Write::MoveMemory { slug, .. }
        | Write::TodoClose { slug, .. } => vec![slug.as_str()],
        Write::UnarchiveMemory {
            slug, restore_as, ..
        } => [Some(slug.as_str()), restore_as.as_deref()]
            .into_iter()
            .flatten()
            .collect(),
        Write::WorkspaceCreate { name, vision } => [Some(name.as_str()), vision.as_deref()]
            .into_iter()
            .flatten()
            .collect(),
        Write::WorkspaceOptimize { .. } | Write::PersonaOptimize { .. } => Vec::new(),
        Write::WorkspaceSnapshot { description, .. } => {
            description.as_deref().into_iter().collect()
        }
    }
}

/// What a write is refused with where a text it carries holds what looks like a credential:
/// a proposal never carries one, so nothing of it is held or shown.
pub const HOLDS_A_CREDENTIAL: &str = "this change carries what looks like a credential, which a \
     proposal never carries, so it was not held and nothing was written: keep the credential in a \
     vault, and ask again without it";

/// What a write is refused with where a text it carries is not text: nothing of it is held.
pub const NOT_TEXT: &str =
    "this change carries something that is not text, so it was not held and nothing was written";

/// **Why `write` is not held for the person**, or none: a text it carries holds what looks
/// like a credential, or is not text.
pub fn refusal(write: &Write) -> Option<&'static str> {
    use crate::harness::asked::edit::unshown;
    use crate::harness::model::{Unshown, Written};
    texts(write).into_iter().find_map(|text| {
        match unshown(&[Written::File {
            content: text.to_owned(),
        }]) {
            Some(Unshown::Credential) => Some(HOLDS_A_CREDENTIAL),
            Some(Unshown::Binary) => Some(NOT_TEXT),
            _ => None,
        }
    })
}

/// Whether every text `write` carries is drawn as it is, character for character: none holds a
/// carriage return or a character that draws as nothing or as something else, so a held write's
/// own tab shows it unescaped, and what the person reads there is what is written.
pub fn drawn_as_it_is(write: &Write) -> bool {
    use crate::harness::asked::edit::unshown;
    use crate::harness::model::{Unshown, Written};
    texts(write).into_iter().all(|text| {
        !text.contains('\r')
            && !purlis_unicode::hides(text, purlis_unicode::Breaks::Kept)
            && matches!(
                unshown(&[Written::File {
                    content: text.to_owned(),
                }]),
                None | Some(Unshown::TooBig)
            )
    })
}

/// **What `write` is, in purlis's words alone**, for the chat's word on it: its store and the
/// memory or todo it names, never a text it carries.
pub fn subject(write: &Write) -> String {
    match write {
        Write::PersonaRemember { shared: true, .. } => "a memory for shared memory".to_owned(),
        Write::PersonaRemember { .. } => "a memory for its persona's memory".to_owned(),
        Write::WorkspaceRemember { .. } => "a memory for this workspace's memory".to_owned(),
        Write::Todo { .. } => "a todo for this workspace".to_owned(),
        Write::WorkspaceVision { .. } => "this workspace's vision".to_owned(),
        Write::WorkspaceSection { section, .. } => {
            format!("this workspace's {}", section.header())
        }
        Write::Forget { of, .. } => format!("forgetting a memory in {}", store(*of)),
        Write::EditMemory { of, .. } => format!("a memory's rewrite in {}", store(*of)),
        Write::ArchiveMemory { of, .. } => format!("archiving a memory in {}", store(*of)),
        Write::UnarchiveMemory { of, .. } => {
            format!("bringing a memory back in {}", store(*of))
        }
        Write::MoveMemory { from, to, .. } => {
            format!("moving a memory from {} to {}", store(*from), store(*to))
        }
        Write::WorkspaceOptimize { .. } => "tidying this workspace's memory".to_owned(),
        Write::PersonaOptimize { of, .. } => format!("tidying {}", store(*of)),
        Write::TodoClose { how, .. } => match how {
            crate::wscmd::todo::Close::Reopen => "reopening a todo".to_owned(),
            _ => "closing a todo".to_owned(),
        },
        Write::WorkspaceCreate { .. } => "a new workspace".to_owned(),
        Write::WorkspaceSnapshot { .. } => "a record of this workspace's branches".to_owned(),
    }
}

fn said_held_to(write: &Write, most: Option<usize>) -> String {
    let shown = |text: &str| shown(text, most);
    let titled = |title: Option<&str>, text: &str| titled(title, text, most);
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
            (Some(title), None) => {
                format!("Retitle memory {slug} in {} as “{title}”", store(*of))
            }
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
        Write::TodoClose { how, slug } => match how {
            crate::wscmd::todo::Close::Reopen => format!("Reopen done todo {slug}"),
            _ => format!("Close todo {slug} ({})", how.verb()),
        },
        Write::WorkspaceCreate { name, vision } => match vision {
            Some(vision) => format!("Make workspace {name}, its vision: {}", shown(vision)),
            None => format!("Make workspace {name}"),
        },
        Write::WorkspaceSnapshot { description, force } => {
            let branches = if *force {
                "Record this workspace's repos and branches as they stand, even those that would not restore"
            } else {
                "Record this workspace's repos and branches"
            };
            match description {
                Some(description) => format!("{branches}, described as: {}", shown(description)),
                None => branches.to_owned(),
            }
        }
    }
}

#[cfg(test)]
mod tests;
