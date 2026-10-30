// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a running chat's skills were when it started (SK-7, #2770): a stamp differs exactly
//! where a chat on that harness would miss the change.

use std::path::Path;

use crate::{Chat, HarnessSkills, Machine, Route, Stamp, skill_set};

use super::Reload;

fn skill(root: &Path, name: &str, body: &str) {
    let folder = root.join("skills").join(name);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Does {name}.\n---\n\n{body}\n"),
    )
    .unwrap();
}

fn stamp(root: &Path, reload: Reload) -> Stamp {
    let set = skill_set(
        Some(root),
        &Machine::default(),
        &Chat {
            workspace: None,
            persona: None,
            names: &[],
            turned_off: &[],
            harness: HarnessSkills {
                route: Route::Plugin,
            },
        },
    );
    Stamp::of(&set, reload)
}

#[test]
fn an_edit_changes_the_stamp_only_where_the_harness_does_not_read_it_again() {
    let tmp = tempfile::tempdir().unwrap();
    skill(tmp.path(), "deploy", "one");
    let (nothing, bodies) = (
        stamp(tmp.path(), Reload::Nothing),
        stamp(tmp.path(), Reload::Bodies),
    );

    skill(tmp.path(), "deploy", "two");

    assert_ne!(stamp(tmp.path(), Reload::Nothing), nothing);
    assert_eq!(stamp(tmp.path(), Reload::Bodies), bodies);
}

#[test]
fn a_skill_added_or_renamed_changes_the_stamp_on_every_harness() {
    let tmp = tempfile::tempdir().unwrap();
    skill(tmp.path(), "deploy", "one");
    let bodies = stamp(tmp.path(), Reload::Bodies);

    skill(tmp.path(), "triage", "one");

    assert_ne!(stamp(tmp.path(), Reload::Bodies), bodies);
    assert_eq!(
        stamp(tmp.path(), Reload::Bodies),
        stamp(tmp.path(), Reload::Bodies)
    );
}

#[test]
fn what_changed_names_the_skills_added_gone_and_edited_since_the_start() {
    let tmp = tempfile::tempdir().unwrap();
    skill(tmp.path(), "deploy", "one");
    skill(tmp.path(), "review", "one");
    skill(tmp.path(), "triage", "one");
    let then = stamp(tmp.path(), Reload::Nothing);

    std::fs::remove_dir_all(tmp.path().join("skills/review")).unwrap();
    skill(tmp.path(), "triage", "two");
    skill(tmp.path(), "zeta", "one");
    let changed = then.changed_to(&stamp(tmp.path(), Reload::Nothing));

    assert_eq!(changed.added, ["zeta"]);
    assert_eq!(changed.gone, ["review"]);
    assert_eq!(changed.edited, ["triage"]);
    assert!(!changed.is_empty());
    assert!(then.changed_to(&then).is_empty(), "a stamp is its own");
}
