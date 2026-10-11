//! The project's harness declarations in Settings (#1389): what is listed, and which file Open
//! file opens.

use super::*;

const GEMINI: &str = r#"
name = "gemini"
title = "Gemini CLI"
program = "gemini"
env = ["GEMCLI_*"]

[session]
chosen_by = "harness"

[terminal]
newline = "\r"
ready_to_type = "never"

[levels]
terminal = true
hooks = false
acp = ["gemini", "--experimental-acp"]
"#;

/// A project holding `harnesses/<file>` with `text`, for each.
fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a directory");
    let folder = dir.path().join(harness_declaration::DIR);
    std::fs::create_dir_all(&folder).expect("a folder");
    for (file, text) in files {
        std::fs::write(folder.join(file), text).expect("written");
    }
    dir
}

#[test]
fn the_project_s_own_declarations_are_listed_and_the_built_ins_are_not() {
    let dir = project(&[("gemini.toml", GEMINI)]);
    let read = listed(&harness_declaration::read(dir.path()));
    assert_eq!(
        read.declared,
        vec![DeclaredHarness {
            name: "gemini".into(),
            title: "Gemini CLI".into(),
            program: "gemini".into(),
            file: "harnesses/gemini.toml".into(),
            acp: true,
        }]
    );
    assert_eq!(read.refused, Vec::new());
}

#[test]
fn a_declaration_purlis_will_not_read_is_listed_with_why_and_opens() {
    let dir = project(&[("broken.toml", "name = ")]);
    let read = listed(&harness_declaration::read(dir.path()));
    assert_eq!(read.declared, Vec::new());
    assert_eq!(read.refused.len(), 1, "{:?}", read.refused);
    assert_eq!(read.refused[0].file, "harnesses/broken.toml");
    assert!(!read.refused[0].reason.is_empty());
    let opened = declaration_file(dir.path(), "harnesses/broken.toml").expect("it opens");
    assert!(
        opened.ends_with("harnesses/broken.toml"),
        "{}",
        opened.display()
    );
}

#[test]
fn only_a_file_the_listing_named_is_opened() {
    let dir = project(&[("gemini.toml", GEMINI)]);
    std::fs::write(dir.path().join("secret.toml"), "x = 1").expect("written");
    for file in [
        "secret.toml",
        "harnesses/../secret.toml",
        "harnesses/none.toml",
        "harnesses/",
    ] {
        let why = declaration_file(dir.path(), file).expect_err(file);
        assert!(
            why.contains("is not one of this project's harness declarations"),
            "{why}"
        );
    }
    assert!(declaration_file(dir.path(), "harnesses/gemini.toml").is_ok());
}

#[cfg(unix)]
#[test]
fn a_declaration_that_links_out_of_the_folder_is_not_opened() {
    let dir = project(&[]);
    std::fs::write(dir.path().join("secret.toml"), GEMINI).expect("written");
    std::os::unix::fs::symlink(
        dir.path().join("secret.toml"),
        dir.path().join("harnesses/linked.toml"),
    )
    .expect("a link");
    // Listed or not (a reader may refuse the link), it is never handed to the editor.
    assert!(declaration_file(dir.path(), "harnesses/linked.toml").is_err());
}

#[test]
fn an_absolute_path_or_one_that_climbs_out_is_never_opened() {
    let dir = project(&[("gemini.toml", GEMINI)]);
    let absolute = dir.path().join("harnesses/gemini.toml");
    for file in [
        absolute.display().to_string(),
        "/etc/hosts".to_owned(),
        "harnesses/../harnesses/gemini.toml".to_owned(),
        "./harnesses/gemini.toml".to_owned(),
        "harnesses//gemini.toml".to_owned(),
    ] {
        assert!(declaration_file(dir.path(), &file).is_err(), "{file}");
    }
}

#[cfg(unix)]
#[test]
fn a_harnesses_folder_that_is_a_link_opens_nothing_even_inside_the_project() {
    let dir = tempfile::tempdir().expect("a directory");
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("a folder");
    std::fs::write(elsewhere.join("gemini.toml"), GEMINI).expect("written");
    std::os::unix::fs::symlink(&elsewhere, dir.path().join(harness_declaration::DIR))
        .expect("a link");
    let read = listed(&harness_declaration::read(dir.path()));
    assert_eq!(
        read.declared,
        Vec::new(),
        "a linked folder declares nothing"
    );
    for refused in &read.refused {
        assert!(
            declaration_file(dir.path(), &refused.file).is_err(),
            "{}",
            refused.file
        );
    }
    assert!(declaration_file(dir.path(), "harnesses/gemini.toml").is_err());
}

#[cfg(unix)]
#[test]
fn a_harnesses_folder_that_links_to_a_file_opens_nothing() {
    let dir = tempfile::tempdir().expect("a directory");
    std::fs::write(dir.path().join("secret.toml"), GEMINI).expect("written");
    std::os::unix::fs::symlink(
        dir.path().join("secret.toml"),
        dir.path().join(harness_declaration::DIR),
    )
    .expect("a link");
    let read = listed(&harness_declaration::read(dir.path()));
    assert_eq!(read.refused.len(), 1, "{:?}", read.refused);
    assert!(declaration_file(dir.path(), &read.refused[0].file).is_err());
}

#[cfg(unix)]
#[test]
fn a_declaration_that_links_to_another_in_the_folder_opens_that_file() {
    let dir = project(&[("gemini.toml", GEMINI)]);
    std::os::unix::fs::symlink(
        dir.path().join("harnesses/gemini.toml"),
        dir.path().join("harnesses/again.toml"),
    )
    .expect("a link");
    // The reader refuses a link, and lists it; Open file opens what it resolves to, which is
    // a declaration of the project's own.
    let read = listed(&harness_declaration::read(dir.path()));
    assert!(
        read.refused
            .iter()
            .any(|one| one.file == "harnesses/again.toml"),
        "{read:?}"
    );
    let opened = declaration_file(dir.path(), "harnesses/again.toml").expect("it opens");
    assert!(
        opened.ends_with("harnesses/gemini.toml"),
        "{}",
        opened.display()
    );
}
