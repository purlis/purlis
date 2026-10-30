// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The earlier names and levels this machine keeps of a skill git cannot follow (#4018).

use std::path::Path;

use super::*;

fn project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("charter.toml"), "schema = 1\n").unwrap();
    tmp
}

fn at(time: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(time)
        .unwrap()
        .with_timezone(&chrono::Utc)
}

#[test]
fn a_local_rename_keeps_the_old_name_and_a_later_move_takes_it_along() {
    let project = project();
    let root = project.path();
    let local = Level::project().local(true);
    let ops = Level::workspace("ops");

    moved(
        root,
        (&local, "deploy"),
        (&local, "ship"),
        &[],
        at("2026-10-30T10:00:00Z"),
    )
    .unwrap();
    assert_eq!(of(root, &local, "deploy"), []);
    let kept = of(root, &local, "ship");
    assert_eq!(
        kept,
        [Earlier {
            name: "deploy".into(),
            level: "skills.local".into(),
            until: "2026-10-30T10:00:00+00:00".into(),
            grades: Vec::new(),
            folder: None,
        }]
    );

    // A move git follows keeps no new place, and takes the earlier ones along.
    moved(
        root,
        (&local, "ship"),
        (&Level::project(), "ship"),
        &[],
        at("2026-10-30T11:00:00Z"),
    )
    .unwrap();
    moved(
        root,
        (&Level::project(), "ship"),
        (&ops, "ship"),
        &[],
        at("2026-10-30T12:00:00Z"),
    )
    .unwrap();
    let places: Vec<(String, String)> = of(root, &ops, "ship")
        .into_iter()
        .map(|one| (one.name, one.level))
        .collect();
    assert_eq!(
        places,
        [
            ("ship".to_owned(), "skills.local".to_owned()),
            ("deploy".to_owned(), "skills.local".to_owned())
        ]
    );
    assert_eq!(of(root, &Level::project(), "ship"), []);

    dropped(root, &ops, "ship").unwrap();
    assert_eq!(of(root, &ops, "ship"), []);
}

#[test]
fn a_change_git_follows_of_a_skill_with_no_earlier_place_keeps_no_state() {
    let project = project();
    let root = project.path();

    moved(
        root,
        (&Level::project(), "deploy"),
        (&Level::project(), "ship"),
        &[],
        at("2026-10-30T10:00:00Z"),
    )
    .unwrap();
    dropped(root, &Level::project(), "ship").unwrap();

    assert!(crate::appstate::dir(root).is_none_or(|dir| !dir.join(FILE).exists()));
}

#[test]
fn an_entry_that_does_not_hold_together_is_left_out() {
    let project = project();
    let root = project.path();
    let dir = crate::appstate::made(root).unwrap();
    std::fs::write(
        dir.join(FILE),
        r#"{"v":1,"skills":{"skills.local/ship":[
            {"name":"../x","level":"skills.local","until":"2026-10-30T10:00:00Z"},
            {"name":"deploy","level":"nowhere","until":"2026-10-30T10:00:00Z"},
            {"name":"deploy","level":"skills.local","until":"yesterday"},
            {"name":"deploy","level":"skills.local","until":"2026-10-30T10:00:00Z"}
        ]}}"#,
    )
    .unwrap();

    let kept = of(root, &Level::project().local(true), "ship");
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].name, "deploy");
}

#[test]
fn a_move_out_of_git_keeps_the_places_git_followed_it_through() {
    let project = project();
    let root = project.path();
    let local = Level::project().local(true);

    moved(
        root,
        (&Level::project(), "deploy"),
        (&local, "deploy"),
        &[("ship".to_owned(), "skills".to_owned())],
        at("2026-10-30T10:00:00Z"),
    )
    .unwrap();

    let places: Vec<(String, String)> = of(root, &local, "deploy")
        .into_iter()
        .map(|one| (one.name, one.level))
        .collect();
    assert_eq!(
        places,
        [
            ("deploy".to_owned(), "skills".to_owned()),
            ("ship".to_owned(), "skills".to_owned())
        ]
    );
}

/// A skill `name` only on this machine, made in the project at `root` as a folder holds it.
fn made(root: &Path, name: &str) {
    let folder = root.join("skills.local").join(name);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: A skill.\n---\n\nStep one.\n"),
    )
    .unwrap();
}

#[cfg(unix)]
#[test]
fn a_place_lends_nothing_to_another_folder_made_at_its_key_and_a_restore_takes_it_back_4104() {
    let project = project();
    let root = project.path();
    let local = Level::project().local(true);
    // `deploy` renamed `ship`: the folder that took the key is kept.
    made(root, "ship");
    moved(
        root,
        (&local, "deploy"),
        (&local, "ship"),
        &[],
        at("2026-10-30T10:00:00Z"),
    )
    .unwrap();
    assert_eq!(of(root, &local, "ship").len(), 1);
    assert!(of(root, &local, "ship")[0].folder.is_some());

    // `ship` removed by hand, and another made at its name, as an install or a draft's accept
    // or a hand would: the old one is moved away first, so the new folder is surely another.
    std::fs::rename(
        root.join("skills.local/ship"),
        root.join("skills.local/.gone"),
    )
    .unwrap();
    made(root, "ship");
    std::fs::remove_dir_all(root.join("skills.local/.gone")).unwrap();
    assert_eq!(
        of(root, &local, "ship"),
        [],
        "another skill's grades are none of its own"
    );

    // A Restore brings back the folder the places are kept for.
    refound(root, &local, "ship").unwrap();
    assert_eq!(of(root, &local, "ship").len(), 1);
}

#[test]
fn an_entry_with_a_grade_or_a_folder_not_purlis_writes_is_left_out_4104() {
    let project = project();
    let root = project.path();
    let dir = crate::appstate::made(root).unwrap();
    std::fs::write(
        dir.join(FILE),
        r#"{"v":1,"skills":{"skills.local/ship":[
            {"name":"a","level":"skills.local","until":"2026-10-30T10:00:00Z","grades":["../x"]},
            {"name":"b","level":"skills.local","until":"2026-10-30T10:00:00Z","folder":"/etc"},
            {"name":"c","level":"skills.local","until":"2026-10-30T10:00:00Z","grades":["g1"]}
        ]}}"#,
    )
    .unwrap();

    let kept = of(root, &Level::project().local(true), "ship");
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].name, "c");
    assert_eq!(kept[0].grades, ["g1"]);
}
