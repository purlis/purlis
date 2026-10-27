// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's conversation carried between two devices' harness folders, as the declarations
//! purlis ships say where each harness keeps one.

use std::path::Path;

use super::*;
use crate::harness::SessionId;
use crate::harness_declaration::Declaration;

fn declared(name: &str) -> Declaration {
    crate::harness_declaration::builtins()
        .iter()
        .find(|one| one.name == name)
        .cloned()
        .expect("a declaration purlis ships")
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

const ID: &str = "0d6f3c1e-8a4b-4c7d-9e21-5b3f2a1c0e99";

#[test]
fn a_claude_code_conversation_is_found_here_and_placed_under_the_folder_its_directory_names_there()
{
    let here = tempfile::tempdir().unwrap();
    let there = tempfile::tempdir().unwrap();
    let claude = declared("claude");
    let id = SessionId::new(ID).unwrap();
    let kept = here
        .path()
        .join(format!("projects/-Users-a-work/{ID}.jsonl"));
    write(&kept, "{\"said\":\"hello\"}\n");

    let found = find(&claude, &Folders::config(here.path()), &id).expect("it is found");
    assert_eq!(found.path, kept);
    assert_eq!(
        found.shape,
        format!("{{config}}/projects/{{cwd_key}}/{ID}.jsonl")
    );

    let cwd = Path::new("/data/projects/01J/workspaces/ship");
    let placed = place_of(
        &claude,
        &Folders::config(there.path()),
        cwd,
        &id,
        &found.shape,
    )
    .expect("it has a place there");
    assert_eq!(
        placed,
        there.path().join(format!(
            "projects/-data-projects-01J-workspaces-ship/{ID}.jsonl"
        ))
    );
}

#[test]
fn of_two_copies_of_a_conversation_the_one_written_last_is_carried() {
    let here = tempfile::tempdir().unwrap();
    let claude = declared("claude");
    let id = SessionId::new(ID).unwrap();
    let older = here.path().join(format!("projects/-old/{ID}.jsonl"));
    let newer = here.path().join(format!("projects/-new/{ID}.jsonl"));
    write(&older, "before\n");
    write(&newer, "after\n");
    let past = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(&older)
        .unwrap()
        .set_modified(past)
        .unwrap();

    let found = find(&claude, &Folders::config(here.path()), &id).unwrap();
    assert_eq!(found.path, newer);
}

#[test]
fn a_conversation_that_is_not_there_is_said_and_not_carried() {
    let here = tempfile::tempdir().unwrap();
    let id = SessionId::new(ID).unwrap();
    let found = find(&declared("claude"), &Folders::config(here.path()), &id);
    assert!(matches!(found, Err(Refused::NotFound(_))), "{found:?}");
}

#[test]
fn codex_and_opencode_conversations_do_not_move_yet_and_say_why() {
    let here = tempfile::tempdir().unwrap();
    let id = SessionId::new(ID).unwrap();
    for harness in ["codex", "opencode"] {
        let found = find(&declared(harness), &Folders::config(here.path()), &id);
        assert!(
            matches!(&found, Err(Refused::DoesNotMove(why)) if why.contains("does not")),
            "{harness}: {found:?}"
        );
    }
}

#[test]
fn a_shape_that_is_not_the_declarations_file_is_refused_whatever_it_climbs_to() {
    let there = tempfile::tempdir().unwrap();
    let claude = declared("claude");
    let id = SessionId::new(ID).unwrap();
    let cwd = Path::new("/w");
    for shape in [
        format!("{{config}}/projects/../{ID}.jsonl"),
        format!("{{config}}/projects/{{cwd_key}}/../../{ID}.jsonl"),
        format!("{{config}}/projects/-some-dir/{ID}.jsonl"),
        "{config}/projects/{cwd_key}/other.jsonl".to_owned(),
        format!("{{data}}/projects/{{cwd_key}}/{ID}.jsonl"),
        format!("{{config}}/settings/{{cwd_key}}/{ID}.jsonl"),
        format!("{{config}}/projects/{{cwd_key}}/{ID}.jsonl/x"),
        "{config}/projects/{cwd_key}/.credentials.json".to_owned(),
        "/etc/passwd".to_owned(),
    ] {
        let placed = place_of(&claude, &Folders::config(there.path()), cwd, &id, &shape);
        assert!(
            matches!(placed, Err(Refused::NotAllowed(_))),
            "{shape}: {placed:?}"
        );
    }
}

#[test]
fn a_kept_conversation_replaces_the_older_copy_and_is_readable_by_this_user_alone() {
    let there = tempfile::tempdir().unwrap();
    let carried = tempfile::tempdir().unwrap();
    let from = carried.path().join("came");
    std::fs::write(&from, "now\n").unwrap();
    let at = there.path().join(format!("projects/-w/{ID}.jsonl"));
    write(&at, "then\n");

    keep(there.path(), &at, &from).unwrap();
    assert_eq!(std::fs::read_to_string(&at).unwrap(), "now\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&at).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "{mode:o}");
    }
}

#[cfg(unix)]
#[test]
fn a_conversation_is_never_kept_through_a_link_in_the_harness_folder() {
    let there = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let carried = tempfile::tempdir().unwrap();
    let from = carried.path().join("came");
    std::fs::write(&from, "now\n").unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), there.path().join("projects")).unwrap();
    let at = there.path().join(format!("projects/-w/{ID}.jsonl"));

    let kept = keep(there.path(), &at, &from);
    assert!(matches!(kept, Err(Refused::NotAllowed(_))), "{kept:?}");
    assert_eq!(std::fs::read_dir(elsewhere.path()).unwrap().count(), 0);
}

fn profile(env: &[(&str, &str)]) -> crate::profiles::Profile {
    crate::profiles::Profile {
        name: "work".into(),
        kind: "claude".into(),
        harness: "claude-code".into(),
        command: vec!["claude".into()],
        env: env
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect(),
        source: crate::profiles::Source::BuiltIn,
    }
}

#[test]
fn a_profiles_folder_is_its_own_variable_then_the_inherited_one_then_the_harness_default() {
    let claude = declared("claude");
    let home = Path::new("/home/a");
    let inherited = |name: &str| (name == "CLAUDE_CONFIG_DIR").then(|| "/inherited".to_owned());
    let nothing = |_: &str| None;

    let own = Folders::for_profile(
        &profile(&[("CLAUDE_CONFIG_DIR", "/profiles/work/config")]),
        &claude,
        home,
        &inherited,
    );
    assert_eq!(
        own,
        Some(Folders::config(Path::new("/profiles/work/config")))
    );
    let from_env = Folders::for_profile(&profile(&[]), &claude, home, &inherited);
    assert_eq!(from_env, Some(Folders::config(Path::new("/inherited"))));
    let default = Folders::for_profile(&profile(&[]), &claude, home, &nothing);
    assert_eq!(default, Some(Folders::config(Path::new("/home/a/.claude"))));
    let relative = Folders::for_profile(
        &profile(&[("CLAUDE_CONFIG_DIR", "config")]),
        &claude,
        home,
        &nothing,
    );
    assert_eq!(relative, None);
}
