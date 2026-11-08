// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use super::*;
use crate::{Effective, Origin, OutsideSkill, Owner, Reason, Route, Skill, SkillSet, Switch};

/// A skill folder `dir/<name>` holding a `SKILL.md` that says `body`, and a script beside it.
fn folder(dir: &Path, name: &str, body: &str) -> PathBuf {
    let at = dir.join(name);
    std::fs::create_dir_all(at.join("scripts")).unwrap();
    std::fs::write(
        at.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Does {name}.\n---\n{body}\n"),
    )
    .unwrap();
    std::fs::write(at.join("scripts/run.sh"), "echo run\n").unwrap();
    at
}

fn skill(at: &Path, name: &str) -> Skill {
    Skill {
        name: name.to_owned(),
        description: format!("Does {name}."),
        path: at.join("SKILL.md"),
    }
}

/// The plane's copy of `name`, in the project's skills, as a chat on Claude Code is handed it.
fn plane(at: &Path, name: &str, origin: Origin) -> Effective {
    Effective {
        root: at.parent().unwrap().to_path_buf(),
        skill: skill(at, name),
        route: Route::Plugin,
        origin,
        local: false,
        deprecated: None,
    }
}

/// `owner`'s copy of `name`, which the harness finds itself.
fn own(at: &Path, name: &str, owner: Owner) -> OutsideSkill {
    OutsideSkill {
        id: match &owner {
            Owner::Plugin(plugin) => format!("{plugin}:{name}"),
            Owner::You | Owner::Repo(_) => name.to_owned(),
        },
        skill: skill(at, name),
        root: at.to_path_buf(),
        origin: Origin::Harness,
        owner,
        switch: Switch::Alone,
        off: false,
        unread: None,
    }
}

fn matt() -> Owner {
    Owner::Plugin("mattpocock-skills".to_owned())
}

#[test]
fn two_folders_are_the_same_only_where_every_file_is() {
    let tmp = tempfile::tempdir().unwrap();
    let a = folder(&tmp.path().join("a"), "tdd", "Red, green, refactor.");
    let b = folder(&tmp.path().join("b"), "tdd", "Red, green, refactor.");
    assert!(same_content(&a, &b));
    std::fs::write(b.join("scripts/run.sh"), "echo other\n").unwrap();
    assert!(!same_content(&a, &b), "a script differs");
    std::fs::write(b.join("scripts/run.sh"), "echo run\n").unwrap();
    std::fs::write(b.join("extra.md"), "more\n").unwrap();
    assert!(!same_content(&a, &b), "one has a file more");
    assert!(
        !same_content(&a, &tmp.path().join("gone")),
        "one is not there"
    );
}

#[cfg(unix)]
#[test]
fn a_folder_with_a_link_in_it_is_never_the_same_as_another() {
    let tmp = tempfile::tempdir().unwrap();
    let a = folder(&tmp.path().join("a"), "tdd", "x");
    let b = folder(&tmp.path().join("b"), "tdd", "x");
    std::os::unix::fs::symlink(a.join("SKILL.md"), a.join("again.md")).unwrap();
    std::os::unix::fs::symlink(b.join("SKILL.md"), b.join("again.md")).unwrap();
    assert!(!same_content(&a, &b));
}

/// #5737: the same skill from two sources takes one slot, and the plane's copy says which it
/// gave way to.
#[test]
fn a_plane_skill_the_same_as_a_harness_copy_is_left_out_naming_the_kept_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let mine = folder(
        &tmp.path().join("plane/skills"),
        "tdd",
        "Red, green, refactor.",
    );
    let theirs = folder(
        &tmp.path().join("plugin/skills"),
        "tdd",
        "Red, green, refactor.",
    );
    let set = SkillSet {
        effective: vec![plane(&mine, "tdd", Origin::Project)],
        ..SkillSet::default()
    };

    let set = set.without_copies_of(&[own(&theirs, "tdd", matt())]);

    assert!(set.effective.is_empty(), "{:?}", set.effective);
    assert_eq!(set.left_out.len(), 1);
    assert_eq!(set.left_out[0].name, "tdd");
    assert_eq!(set.left_out[0].reason, Reason::SameAs { kept: matt() });
    assert_eq!(
        set.left_out[0].reason.to_string(),
        "the same as the copy plugin mattpocock-skills brings, which is listed in its place"
    );
    assert_eq!(
        Reason::SameAs { kept: Owner::You }.to_string(),
        "the same as your own copy, which is listed in its place"
    );
}

/// #5737: purlis never picks a winner between copies that differ, and a built-in skill never
/// gives way.
#[test]
fn a_differing_copy_and_a_built_in_skill_are_both_kept() {
    let tmp = tempfile::tempdir().unwrap();
    let mine = folder(
        &tmp.path().join("plane/skills"),
        "tdd",
        "Red, green, refactor.",
    );
    let theirs = folder(
        &tmp.path().join("plugin/skills"),
        "tdd",
        "Write the test after.",
    );
    let built_in = folder(&tmp.path().join("bundle/skills"), "purlis-secrets", "Same.");
    let copied = folder(&tmp.path().join("you"), "purlis-secrets", "Same.");
    let set = SkillSet {
        effective: vec![
            plane(&built_in, "purlis-secrets", Origin::BuiltIn),
            plane(&mine, "tdd", Origin::Project),
        ],
        ..SkillSet::default()
    };

    let kept = set.clone().without_copies_of(&[
        own(&theirs, "tdd", matt()),
        own(&copied, "purlis-secrets", Owner::You),
    ]);

    assert_eq!(kept, set);
}

/// #5737: copies of one name that differ, wherever they come from, are named in pairs; the
/// same copy twice, a repo's (whose narrower copy wins, decision 51) and one turned off are not.
#[test]
fn differing_copies_of_one_name_are_named_in_pairs() {
    let tmp = tempfile::tempdir().unwrap();
    let mine = folder(
        &tmp.path().join("plane/skills"),
        "tdd",
        "Red, green, refactor.",
    );
    let theirs = folder(
        &tmp.path().join("plugin/skills"),
        "tdd",
        "Write the test after.",
    );
    let yours = folder(&tmp.path().join("you"), "tdd", "Red, green, refactor.");
    let review = folder(&tmp.path().join("plugin/skills"), "review", "One way.");
    let other_review = folder(&tmp.path().join("other/skills"), "review", "Another way.");
    let off = folder(&tmp.path().join("off/skills"), "review", "A third way.");
    let set = SkillSet {
        effective: vec![plane(&mine, "tdd", Origin::Project)],
        ..SkillSet::default()
    };
    let mut turned_off = own(&off, "review", Owner::Plugin("off".to_owned()));
    turned_off.off = true;
    let outside = [
        own(&theirs, "tdd", matt()),
        own(&yours, "tdd", Owner::You),
        own(&review, "review", matt()),
        own(&other_review, "review", Owner::Plugin("other".to_owned())),
        turned_off,
    ];

    let pairs = differing_copies(&set, &outside);

    let said: Vec<(String, String, String)> = pairs
        .iter()
        .map(|pair| {
            (
                pair.name.clone(),
                pair.one.to_string(),
                pair.other.to_string(),
            )
        })
        .collect();
    assert_eq!(
        said,
        vec![
            (
                "review".to_owned(),
                "plugin mattpocock-skills".to_owned(),
                "plugin other".to_owned()
            ),
            (
                "tdd".to_owned(),
                "the project's".to_owned(),
                "plugin mattpocock-skills".to_owned()
            ),
            (
                "tdd".to_owned(),
                "plugin mattpocock-skills".to_owned(),
                "your own".to_owned()
            ),
        ],
        "the project's and your own are the same, so they are no pair"
    );
}
