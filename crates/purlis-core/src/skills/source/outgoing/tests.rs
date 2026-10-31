// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What leaves the machine when a skill is exported or published (SK-18, #2781), and the export.

use std::path::Path;

use super::*;
use crate::skills::verbs::Level;

const SKILL: &str = "---\nname: deploy\ndescription: Deploys it\n---\nDeploy.\n";

/// A skill `name` at the project level of `plane`, with `files` beside its `SKILL.md`.
fn skill_at(plane: &Path, level: &Level, name: &str, text: &str, files: &[(&str, &str)]) {
    let dir = level.dir(plane).unwrap().join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), text).unwrap();
    for (path, bytes) in files {
        let file = dir.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, bytes).unwrap();
    }
}

fn gathered(plane: &Path, purpose: Purpose) -> Outgoing {
    gather(plane, None, &Level::project(), "deploy", purpose).unwrap()
}

fn paths(out: &Outgoing) -> Vec<String> {
    out.skills[0].files.iter().map(OutFile::path).collect()
}

#[test]
fn a_skills_files_go_out_with_its_scripts_still_executable() {
    use std::os::unix::fs::PermissionsExt as _;
    let plane = tempfile::tempdir().unwrap();
    skill_at(
        plane.path(),
        &Level::project(),
        "deploy",
        SKILL,
        &[("scripts/go.sh", "echo hi\n")],
    );
    let script = Level::project()
        .dir(plane.path())
        .unwrap()
        .join("deploy")
        .join("scripts/go.sh");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let out = gathered(plane.path(), Purpose::Export);

    assert_eq!(paths(&out), ["SKILL.md", "scripts/go.sh"]);
    assert!(out.skills[0].files[1].executable);
    assert!(out.stops.is_empty(), "{:?}", out.stops);
}

#[test]
fn a_link_and_a_credential_file_stay_behind_and_are_said() {
    let plane = tempfile::tempdir().unwrap();
    skill_at(
        plane.path(),
        &Level::project(),
        "deploy",
        SKILL,
        &[(".env", "A=1\n"), ("id_rsa", "x\n")],
    );
    let dir = Level::project().dir(plane.path()).unwrap().join("deploy");
    std::os::unix::fs::symlink("/etc/hosts", dir.join("hosts")).unwrap();

    let out = gathered(plane.path(), Purpose::Publish);

    assert_eq!(paths(&out), ["SKILL.md"]);
    let left: Vec<&str> = out.skills[0]
        .left
        .iter()
        .map(|one| one.path.as_str())
        .collect();
    assert_eq!(left, [".env", "hosts", "id_rsa"]);
}

#[test]
fn a_secret_in_a_file_stops_both_and_its_value_is_never_said() {
    let token = format!("ghp_{}", "a1B2".repeat(9));
    for purpose in [Purpose::Export, Purpose::Publish] {
        let plane = tempfile::tempdir().unwrap();
        let text = format!("{SKILL}Use {token} here.\n");
        skill_at(plane.path(), &Level::project(), "deploy", &text, &[]);

        let out = gathered(plane.path(), purpose);

        assert_eq!(out.stops.len(), 1, "{:?}", out.stops);
        let said = out.stops[0].to_string();
        assert!(said.starts_with("deploy/SKILL.md:6"), "{said}");
        assert!(!said.contains(&token), "{said}");
    }
}

#[test]
fn personal_data_and_a_home_path_stop_a_publish_but_not_an_export() {
    let plane = tempfile::tempdir().unwrap();
    let text = format!("{SKILL}Mail ada@example.org and read /Users/ada/notes.md.\n");
    skill_at(plane.path(), &Level::project(), "deploy", &text, &[]);

    assert!(gathered(plane.path(), Purpose::Export).stops.is_empty());
    assert_eq!(gathered(plane.path(), Purpose::Publish).stops.len(), 2);
}

#[test]
fn a_local_skill_is_exported_but_never_published() {
    let plane = tempfile::tempdir().unwrap();
    let local = Level::project().local(true);
    skill_at(plane.path(), &local, "deploy", SKILL, &[]);

    assert!(
        gather(plane.path(), None, &local, "deploy", Purpose::Export)
            .unwrap()
            .stops
            .is_empty()
    );
    let out = gather(plane.path(), None, &local, "deploy", Purpose::Publish).unwrap();
    assert!(
        out.stops[0].to_string().contains("this machine's alone"),
        "{:?}",
        out.stops
    );
}

#[test]
fn a_skill_it_refers_to_comes_along_and_a_broken_reference_is_said() {
    let plane = tempfile::tempdir().unwrap();
    let text = format!("{SKILL}Then use skills:notify and skills:gone.\n");
    skill_at(plane.path(), &Level::project(), "deploy", &text, &[]);
    skill_at(
        plane.path(),
        &Level::project(),
        "notify",
        "---\nname: notify\ndescription: N\n---\n",
        &[],
    );

    let out = gathered(plane.path(), Purpose::Export);

    let names: Vec<&str> = out.skills.iter().map(|one| one.name.as_str()).collect();
    assert_eq!(names, ["deploy", "notify"]);
    assert_eq!(out.broken, ["gone"]);
}

#[test]
fn the_digest_moves_when_a_byte_moves() {
    let plane = tempfile::tempdir().unwrap();
    skill_at(plane.path(), &Level::project(), "deploy", SKILL, &[]);
    let before = gathered(plane.path(), Purpose::Publish).digest();
    skill_at(
        plane.path(),
        &Level::project(),
        "deploy",
        &format!("{SKILL}More.\n"),
        &[],
    );
    assert_ne!(before, gathered(plane.path(), Purpose::Publish).digest());
}

#[test]
fn export_to_a_folder_writes_a_folder_each_skill_works_from_and_never_over_anything() {
    let plane = tempfile::tempdir().unwrap();
    skill_at(
        plane.path(),
        &Level::project(),
        "deploy",
        SKILL,
        &[("refs/a.md", "A\n")],
    );
    let out = gathered(plane.path(), Purpose::Export);
    let to = tempfile::tempdir().unwrap();
    let folder = to.path().join("deploy-export");

    export_folder(&out, &folder).unwrap();

    assert_eq!(
        std::fs::read_to_string(folder.join("deploy/SKILL.md")).unwrap(),
        SKILL
    );
    assert_eq!(
        std::fs::read_to_string(folder.join("deploy/refs/a.md")).unwrap(),
        "A\n"
    );
    let again = export_folder(&out, &folder).unwrap_err();
    assert!(again.contains("already"), "{again}");
}

#[test]
fn export_to_a_zip_holds_the_same_files_and_the_same_files_are_the_same_bytes() {
    let plane = tempfile::tempdir().unwrap();
    skill_at(
        plane.path(),
        &Level::project(),
        "deploy",
        SKILL,
        &[("refs/a.md", "A\n")],
    );
    let out = gathered(plane.path(), Purpose::Export);
    let to = tempfile::tempdir().unwrap();
    let (one, two) = (to.path().join("1.zip"), to.path().join("2.zip"));

    export_zip(&out, &one).unwrap();
    export_zip(&out, &two).unwrap();

    let bytes = std::fs::read(&one).unwrap();
    assert_eq!(bytes, std::fs::read(&two).unwrap());
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut names: Vec<String> = zip
        .file_names()
        .map(|name| name.unwrap().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["deploy/SKILL.md", "deploy/refs/a.md"]);
    let mut text = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("deploy/SKILL.md").unwrap(), &mut text).unwrap();
    assert_eq!(text, SKILL);
    assert!(export_zip(&out, &one).unwrap_err().contains("already"));
}

#[test]
fn nothing_is_exported_while_something_stops_it() {
    let plane = tempfile::tempdir().unwrap();
    let text = format!("{SKILL}ghp_{}\n", "a1B2".repeat(9));
    skill_at(plane.path(), &Level::project(), "deploy", &text, &[]);
    let out = gathered(plane.path(), Purpose::Export);
    let to = tempfile::tempdir().unwrap();

    assert!(export_folder(&out, &to.path().join("f")).is_err());
    assert!(export_zip(&out, &to.path().join("z.zip")).is_err());
    assert!(!to.path().join("f").exists() && !to.path().join("z.zip").exists());
}
