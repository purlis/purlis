// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The earlier names and levels this machine keeps of a skill git cannot follow (#4018).

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
