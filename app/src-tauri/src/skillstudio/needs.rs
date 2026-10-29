// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What in a studio's skills needs the person** (#3689, spec #3679, the amendment's S9): the
//! Needs you page's items, grouped by kind, each with why in one line and one fix. The core
//! works them out from what the studio already read, so the page, the navigation's count and
//! the summary lines (#3695) count the same items.
//!
//! - **Changes to review:** a skill's folder a chat made while it ran, which waits for Accept.
//! - **Clashes:** a harness's or a repo's skill of the name of one purlis hands the chat. A
//!   repo skill of a built-in skill's name is one too: the built-in is guarded, so the fix is
//!   the other copy's switch.
//! - **Problems in a skill:** what the built-in lints say of a skill from outside the plane that
//!   its harness can leave out, so there is something to do about it.
//! - **Pins no chat reads:** the scope's pins, where a chat reads none of them.
//! - **Tidy up:** the project's skills still where only some programs read them, and the old
//!   `[harness_plugins]` keys nothing reads now.

use serde::Serialize;
use specta::Type;

use super::{Level, Owner, SkillSwitch, Studio};
use crate::skillchange::SkillPinner;

/// **What kind of decision an item is**, in the order the page groups them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum NeedKind {
    Review,
    Clash,
    Problem,
    Pins,
    TidyUp,
}

/// **The one fix an item offers**, as the window runs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum NeedFix {
    /// Accept the folder a chat made (`skill_level_accept`).
    Accept { folder: String },
    /// Rename the plane's copy of a skill: its page in the Library, with Rename open.
    Rename {
        name: String,
        level: Level,
        local: bool,
    },
    /// The Harness skills page, where the other copy's switch is.
    Harness,
    /// Take the pin `name` out of `by`'s pins (`skill_pin`, off).
    Unpin { by: SkillPinner, name: String },
    /// The Library, where each pinned skill says so and its pin can be taken out.
    Library,
    /// Move the project's skills where every program reads them: the doctor's fix, by its id
    /// (`plane_doctor_fix`).
    MoveSkills { fix: String },
    /// Propose the files without the old keys, for the person to apply
    /// (`remove_old_harness_plugins`); or leave them, which says so once
    /// (`dismiss_old_harness_plugins`).
    RemoveOldKeys,
    /// Leave the old keys, which no file can be proposed without: said once
    /// (`dismiss_old_harness_plugins`).
    LeaveOldKeys,
}

/// **One item of Needs you**: what it is about, why it needs the person, and its fix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct NeedsYou {
    /// Unique among a studio's items, and the same from one read to the next: what the page
    /// keeps chosen.
    pub id: String,
    pub kind: NeedKind,
    /// What it is about: a skill's name, or what the tidy-up is.
    pub title: String,
    /// Where it is: the skill's level as a person reads it, or the file. `None` where nothing
    /// more is worth saying.
    pub tag: Option<String>,
    /// Why it needs the person, in sentences.
    pub reason: String,
    pub fix: NeedFix,
    /// The fix's words, on its button.
    pub fix_label: String,
    /// What pressing it does, in a sentence.
    pub fix_does: String,
}

/// `said` as a sentence: its first letter upper case, and a full stop where it has none.
fn sentence(said: &str) -> String {
    let mut chars = said.trim().chars();
    let mut out: String = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => return String::new(),
    };
    if !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

/// Whose a skill from outside the plane is, as its tag says it.
fn whose(owner: &Owner) -> String {
    match owner {
        Owner::You => "yours".to_owned(),
        Owner::Plugin { name } => format!("plugin {name}"),
        Owner::Repo { name } => format!("repo {name}"),
    }
}

/// **The items of `studio`**, by kind, from what it already read: no file is read here. The old
/// keys are the project's, read where the studio is answered ([`old_keys`]).
pub(crate) fn of(studio: &Studio) -> Vec<NeedsYou> {
    let mut needs = Vec::new();

    // A folder a chat made while it ran, which waits for the person's Accept (SK-9).
    for skill in &studio.skills {
        if let Some(folder) = &skill.accept {
            needs.push(NeedsYou {
                id: format!("review:{folder}"),
                kind: NeedKind::Review,
                title: skill.name.clone(),
                tag: Some(skill.label.clone()),
                reason: format!(
                    "{} Chats get it once you accept it.",
                    sentence(
                        skill
                            .left_out
                            .as_deref()
                            .unwrap_or("A chat made it while it ran")
                    )
                ),
                fix: NeedFix::Accept {
                    folder: folder.clone(),
                },
                fix_label: "Accept it".to_owned(),
                fix_does: "Commits it, so chats started after that get it.".to_owned(),
            });
        }
    }

    // One item per copy of the plane's that clashes, whichever harnesses load the other.
    let mut seen = std::collections::HashSet::new();
    for clash in &studio.clashes {
        let id = format!(
            "clash:{:?}{}:{}",
            clash.level,
            if clash.local { "+local" } else { "" },
            clash.name
        );
        if !seen.insert(id.clone()) {
            continue;
        }
        let built_in = clash.level == Level::BuiltIn;
        needs.push(NeedsYou {
            id,
            kind: NeedKind::Clash,
            title: clash.name.clone(),
            tag: Some(purlis_core::skills::level_shown(
                level_origin(clash.level),
                clash.local,
            )),
            reason: if built_in {
                format!(
                    "{} The built-in skill can't be renamed or turned off, so the other copy is \
                     the one to switch off for this project.",
                    sentence(&clash.said)
                )
            } else {
                sentence(&clash.said)
            },
            fix: if built_in {
                NeedFix::Harness
            } else {
                NeedFix::Rename {
                    name: clash.name.clone(),
                    level: clash.level,
                    local: clash.local,
                }
            },
            fix_label: if built_in {
                "Show in Harness skills".to_owned()
            } else {
                "Rename it…".to_owned()
            },
            fix_does: if built_in {
                format!(
                    "Opens Harness skills, where the copy of {} has its switch.",
                    whose(&clash.owner)
                )
            } else {
                "Opens its page with Rename, so only one copy keeps the name.".to_owned()
            },
        });
    }

    // What the lints say of a skill from outside the plane its harness can leave out.
    let mut seen = std::collections::HashSet::new();
    for harness in &studio.harnesses {
        for skill in &harness.skills {
            let switchable = matches!(skill.switch, Some(SkillSwitch::Alone));
            if skill.lint.is_empty() || !switchable || !skill.on {
                continue;
            }
            let owner = skill.owner.as_ref().map(whose);
            let id = format!("problem:{}:{}", owner.as_deref().unwrap_or(""), skill.name);
            if !seen.insert(id.clone()) {
                continue;
            }
            let lines: Vec<String> = skill.lint.iter().map(|one| sentence(one)).collect();
            needs.push(NeedsYou {
                id,
                kind: NeedKind::Problem,
                title: skill.name.clone(),
                tag: owner,
                reason: lines.join(" "),
                fix: NeedFix::Harness,
                fix_label: "Show in Harness skills".to_owned(),
                fix_does: format!(
                    "Opens Harness skills, where {} has its switch for this project.",
                    skill.name
                ),
            });
        }
    }

    // The scope's pins, where a chat reads none of them (SK-15d).
    if let Some(refused) = &studio.pins_refused {
        let (at, name) = match &refused.by {
            SkillPinner::Workspace { name } => ("workspace", name),
            SkillPinner::Persona { name } => ("persona", name),
        };
        let pinner = format!("{at} {name}");
        let reason = format!(
            "A chat reads none of {pinner}'s pins: {}, so a router may hide what they pin.",
            refused.why.trim_end_matches('.')
        );
        needs.push(NeedsYou {
            id: format!("pins:{pinner}"),
            kind: NeedKind::Pins,
            title: format!("{pinner}'s pins"),
            tag: Some(refused.file.clone()),
            reason,
            fix: match &refused.bad {
                Some(bad) => NeedFix::Unpin {
                    by: refused.by.clone(),
                    name: bad.clone(),
                },
                None => NeedFix::Library,
            },
            fix_label: match &refused.bad {
                Some(_) => "Remove that pin".to_owned(),
                None => "Unpin some in the Library".to_owned(),
            },
            fix_does: match &refused.bad {
                Some(bad) => format!("Takes {bad:?} out of the pins, in one commit."),
                None => "Opens the Library, where each pinned skill says so.".to_owned(),
            },
        });
    }

    // The project's skills where only some programs read them (SK-21).
    if let Some(unmoved) = &studio.first.unmoved {
        needs.push(NeedsYou {
            id: "tidy-up:unmoved".to_owned(),
            kind: NeedKind::TidyUp,
            title: format!(
                "{} only some programs read",
                purlis_core::counts::counted(unmoved.skills, "skill", "skills")
            ),
            tag: None,
            reason: format!(
                "This project keeps {} where only some chat programs read {}. Moving {} gives \
                 every chat each one, whichever program it runs on. The commit reaches everyone \
                 who works in the project, and a teammate on an older purlis opens the project \
                 read-only until they update.",
                purlis_core::counts::counted(unmoved.skills, "skill", "skills"),
                purlis_core::counts::plural(unmoved.skills, "it", "them"),
                purlis_core::counts::plural(unmoved.skills, "it", "them"),
            ),
            fix: NeedFix::MoveSkills {
                fix: unmoved.fix.clone(),
            },
            fix_label: format!(
                "Move {}",
                purlis_core::counts::plural(unmoved.skills, "it", "them")
            ),
            fix_does: "One commit, which can be undone.".to_owned(),
        });
    }

    needs.sort_by_key(|one| one.kind);
    needs
}

/// **The old `[harness_plugins]` keys, as an item** (#3467): the Ask the Inbox drew while the
/// studio is off.
pub(crate) fn old_keys(old: &crate::oldplugins::OldHarnessPlugins) -> NeedsYou {
    NeedsYou {
        id: "tidy-up:old-plugin-keys".to_owned(),
        kind: NeedKind::TidyUp,
        title: "Old plugin keys".to_owned(),
        tag: Some(old.files.join(", ")),
        reason: old.said.clone(),
        fix: if old.removable {
            NeedFix::RemoveOldKeys
        } else {
            NeedFix::LeaveOldKeys
        },
        fix_label: if old.removable {
            "Remove them…".to_owned()
        } else {
            "Leave them".to_owned()
        },
        fix_does: if old.removable {
            "Proposes each file without them, for you to apply; nothing is written before."
                .to_owned()
        } else {
            "purlis can't propose a file without them: edit it yourself, or leave them, which \
             asks no more."
                .to_owned()
        },
    }
}

/// `items` with the old keys' item among them where there is one, in the page's order.
pub(crate) fn with_old_keys(
    mut items: Vec<NeedsYou>,
    old: Option<&crate::oldplugins::OldHarnessPlugins>,
) -> Vec<NeedsYou> {
    items.extend(old.map(old_keys));
    items.sort_by_key(|one| one.kind);
    items
}

/// The level of the plane a studio's level is, for its words.
fn level_origin(level: Level) -> purlis_core::skills::Origin {
    match level {
        Level::BuiltIn => purlis_core::skills::Origin::BuiltIn,
        Level::Project => purlis_core::skills::Origin::Project,
        Level::Workspace => purlis_core::skills::Origin::Workspace,
        Level::Persona => purlis_core::skills::Origin::Persona,
        Level::Installed => purlis_core::skills::Origin::Harness,
        Level::Repo => purlis_core::skills::Origin::Repo,
    }
}

#[cfg(test)]
mod tests;
