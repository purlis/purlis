// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn folder() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("deploy");
    std::fs::create_dir_all(&folder).unwrap();
    (tmp, folder)
}

fn beside_text(folder: &Path) -> String {
    std::fs::read_to_string(crate::beside(folder)).unwrap()
}

#[test]
fn no_file_beside_the_folder_is_an_active_skill_with_no_owners() {
    let (_tmp, folder) = folder();
    assert_eq!(about_of(&folder), Ok(About::default()));
}

#[test]
fn each_state_is_written_beside_the_folder_and_read_back_keeping_the_grant() {
    let (_tmp, folder) = folder();
    std::fs::write(crate::beside(&folder), "# who gets it\ngrant = \"all\"\n").unwrap();

    let deprecated = Lifecycle::Deprecated(Deprecated {
        replacement: Some("ship".to_owned()),
    });
    set_lifecycle(&folder, &deprecated).unwrap();
    assert_eq!(about_of(&folder).unwrap().lifecycle, deprecated);
    assert!(beside_text(&folder).contains("# who gets it\ngrant = \"all\"\n"));

    set_lifecycle(&folder, &Lifecycle::Archived).unwrap();
    assert_eq!(about_of(&folder).unwrap().lifecycle, Lifecycle::Archived);
    assert!(!beside_text(&folder).contains(REPLACEMENT));
    assert!(archived(&folder));

    set_lifecycle(&folder, &Lifecycle::Active).unwrap();
    assert_eq!(beside_text(&folder), "# who gets it\ngrant = \"all\"\n");
}

#[test]
fn making_a_skill_active_again_removes_a_file_that_held_nothing_else() {
    let (_tmp, folder) = folder();
    set_lifecycle(&folder, &Lifecycle::Archived).unwrap();
    set_lifecycle(&folder, &Lifecycle::Active).unwrap();
    assert!(crate::beside(&folder).symlink_metadata().is_err());
}

#[test]
fn owners_are_written_and_read_and_a_name_that_is_no_owner_is_refused() {
    let (_tmp, folder) = folder();
    let owners = vec!["alice".to_owned(), "bob@example.com".to_owned()];
    set_owners(&folder, &owners).unwrap();
    assert_eq!(about_of(&folder).unwrap().owners, owners);

    let refused = set_owners(&folder, &["two words".to_owned()]).unwrap_err();
    assert!(refused.contains("cannot be an owner"), "{refused}");
    assert_eq!(about_of(&folder).unwrap().owners, owners);

    let many: Vec<String> = (0..=MOST_OWNERS).map(|n| format!("o{n}")).collect();
    assert!(set_owners(&folder, &many).is_err());

    set_owners(&folder, &[]).unwrap();
    assert!(crate::beside(&folder).symlink_metadata().is_err());
}

#[test]
fn a_file_that_is_not_an_allow_listed_value_is_refused_with_a_sentence() {
    let (_tmp, folder) = folder();
    for (text, says) in [
        ("lifecycle = 3\n", "which is no word"),
        ("lifecycle = \"gone\"\n", "none of"),
        (
            "lifecycle = \"deprecated\"\nreplacement = \"Not A Name\"\n",
            "no skill's name",
        ),
        ("owners = \"alice\"\n", "not a list of names"),
        ("owners = [\"a\\u202eb\"]\n", "no owner's name"),
        ("owners = [1]\n", "no owner's name"),
        ("lifecycle = [\n", "not TOML"),
    ] {
        std::fs::write(crate::beside(&folder), text).unwrap();
        let why = about_of(&folder).unwrap_err();
        assert!(why.contains(says), "{text:?}: {why}");
        assert!(!archived(&folder));
    }
}

#[test]
fn a_file_too_large_to_be_one_is_not_read() {
    let (_tmp, folder) = folder();
    let big = format!(
        "# {}\nlifecycle = \"archived\"\n",
        "x".repeat(MOST_BESIDE as usize)
    );
    std::fs::write(crate::beside(&folder), big).unwrap();
    assert!(about_of(&folder).unwrap_err().contains("larger than"));
    assert!(!archived(&folder));
}

#[cfg(unix)]
#[test]
fn a_link_beside_the_folder_is_neither_read_nor_written_through() {
    let (tmp, folder) = folder();
    let elsewhere = tmp.path().join("elsewhere.toml");
    std::fs::write(&elsewhere, "lifecycle = \"archived\"\n").unwrap();
    std::os::unix::fs::symlink(&elsewhere, crate::beside(&folder)).unwrap();

    assert!(about_of(&folder).unwrap_err().contains("not a plain file"));
    assert!(set_lifecycle(&folder, &Lifecycle::Active).is_err());
    assert_eq!(
        std::fs::read_to_string(&elsewhere).unwrap(),
        "lifecycle = \"archived\"\n"
    );
}

#[test]
fn a_file_that_is_not_toml_is_left_as_it_was_by_a_write() {
    let (_tmp, folder) = folder();
    std::fs::write(crate::beside(&folder), "grant = [\n").unwrap();
    let refused = set_lifecycle(&folder, &Lifecycle::Archived).unwrap_err();
    assert!(refused.contains("not TOML"), "{refused}");
    assert_eq!(beside_text(&folder), "grant = [\n");
}
