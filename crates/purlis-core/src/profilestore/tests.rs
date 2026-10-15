// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::path::{Path, PathBuf};

use super::*;

/// A machine: a home with the harness folders a test puts in it, and a config home of its own.
struct Machine {
    _dir: tempfile::TempDir,
    home: PathBuf,
    config: PathBuf,
}

impl Machine {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let config = dir.path().join("config");
        fs::create_dir_all(&home).unwrap();
        Self {
            _dir: dir,
            home,
            config,
        }
    }

    fn folder(&self, relative: &str) -> PathBuf {
        let at = self.home.join(relative);
        fs::create_dir_all(&at).unwrap();
        at
    }

    fn upgrade(&self, env: &[(&str, &str)]) -> Vec<MachineProfile> {
        let env: Vec<(String, String)> = env
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        adopt(&self.config, &self.home, &|name: &str| {
            env.iter()
                .find(|(n, _)| n == name)
                .map(|(_, value)| value.clone())
        })
        .expect("the store is writable")
        .adopted
    }

    fn adoption(&self, env: &[(&str, &str)]) -> Result<Adoption, String> {
        let env: Vec<(String, String)> = env
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        adopt(&self.config, &self.home, &|name: &str| {
            env.iter()
                .find(|(n, _)| n == name)
                .map(|(_, value)| value.clone())
        })
    }

    fn listed(&self) -> Vec<MachineProfile> {
        list(&self.config).expect("the store reads")
    }
}

fn adopted_names(profiles: &[MachineProfile]) -> Vec<(&str, &str)> {
    profiles
        .iter()
        .map(|p| (p.name.as_str(), p.title.as_str()))
        .collect()
}

#[test]
fn each_harness_folder_on_the_machine_becomes_a_profile_named_after_its_harness() {
    let machine = Machine::new();
    let claude = machine.folder(".claude");
    let codex = machine.folder(".codex");
    machine.folder(".local/share/opencode");

    let adopted = machine.upgrade(&[]);

    let here = this_device();
    assert_eq!(
        adopted_names(&adopted),
        vec![
            ("claude", format!("Claude Code ({here})").as_str()),
            ("opencode", format!("opencode ({here})").as_str()),
            ("codex", format!("Codex ({here})").as_str()),
        ]
    );
    assert_eq!(adopted[0].folders.config, claude);
    assert_eq!(adopted[2].folders.config, codex);
    // opencode's variables name the XDG folders it keeps its own `opencode` folder under.
    assert_eq!(adopted[1].folders.config, machine.home.join(".config"));
    assert_eq!(
        adopted[1].folders.data.as_deref(),
        Some(machine.home.join(".local/share").as_path())
    );
    assert!(
        adopted
            .iter()
            .all(|p| p.adopted.as_ref().is_some_and(|a| a.variables.is_empty())),
        "a folder found where the harness looks by default is reached with no variable set: \
         {adopted:?}"
    );
    assert_eq!(
        machine.listed(),
        adopted,
        "what was adopted is what the store lists"
    );
}

#[test]
fn the_folder_the_harnesss_own_variable_names_is_the_one_adopted() {
    let machine = Machine::new();
    machine.folder(".claude");
    let work = machine.folder("work/claude");

    let adopted = machine.upgrade(&[("CLAUDE_CONFIG_DIR", work.to_str().unwrap())]);

    assert_eq!(adopted.len(), 1, "{adopted:?}");
    assert_eq!(adopted[0].folders.config, work);
    assert_eq!(
        adopted[0].adopted.as_ref().unwrap().variables,
        vec!["CLAUDE_CONFIG_DIR".to_owned()],
        "reached again by setting the variable it was found by"
    );
}

#[test]
fn a_harness_with_no_folder_on_this_machine_gets_no_profile() {
    let machine = Machine::new();
    // A variable naming a folder that is not there is not a folder this machine uses.
    let adopted = machine.upgrade(&[("CODEX_HOME", "/nowhere/codex")]);

    assert_eq!(adopted, Vec::new());
    assert_eq!(machine.listed(), Vec::new());
    assert!(
        !path(&machine.config).exists(),
        "nothing to adopt writes no store"
    );
}

#[test]
fn a_second_upgrade_adopts_nothing_and_leaves_the_store_as_it_was() {
    let machine = Machine::new();
    machine.folder(".claude");
    machine.folder(".codex");
    let first = machine.upgrade(&[]);
    let bytes = fs::read(path(&machine.config)).unwrap();

    let second = machine.upgrade(&[]);

    assert_eq!(second, Vec::new());
    assert_eq!(fs::read(path(&machine.config)).unwrap(), bytes);
    assert_eq!(machine.listed(), first);
}

#[cfg(unix)]
#[test]
fn a_folder_a_machine_profile_already_points_at_is_not_adopted_again() {
    let machine = Machine::new();
    let claude = machine.folder(".claude");
    machine.upgrade(&[]);

    // The same folder, spelled through a link: still the one folder.
    let link = machine.home.join("claude-link");
    std::os::unix::fs::symlink(&claude, &link).unwrap();
    let again = machine.upgrade(&[("CLAUDE_CONFIG_DIR", link.to_str().unwrap())]);

    assert_eq!(again, Vec::new());
    assert_eq!(machine.listed().len(), 1);
}

#[test]
fn adoption_writes_nothing_in_the_harnesss_folder() {
    let machine = Machine::new();
    let claude = machine.folder(".claude");
    let transcript = machine.folder(".claude/projects/-p").join("a.jsonl");
    fs::write(&transcript, "{\"type\":\"user\"}\n").unwrap();
    let before = tree(&claude);

    machine.upgrade(&[]);

    assert_eq!(tree(&claude), before);
}

#[test]
fn a_store_purlis_cannot_read_is_never_written_over() {
    let machine = Machine::new();
    machine.folder(".claude");
    fs::create_dir_all(path(&machine.config).parent().unwrap()).unwrap();
    fs::write(path(&machine.config), "not json").unwrap();

    let refused = machine.adoption(&[]);

    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(
        fs::read_to_string(path(&machine.config)).unwrap(),
        "not json"
    );
}

#[test]
fn a_record_this_purlis_cannot_read_survives_an_adoption() {
    let machine = Machine::new();
    machine.folder(".claude");
    fs::create_dir_all(path(&machine.config).parent().unwrap()).unwrap();
    fs::write(
        path(&machine.config),
        r#"{"version":1,"profiles":[{"name":"later","shape":"of a later purlis"}]}"#,
    )
    .unwrap();

    machine.upgrade(&[]);

    let text = fs::read_to_string(path(&machine.config)).unwrap();
    assert!(text.contains("of a later purlis"), "{text}");
    assert_eq!(adopted_names(&machine.listed()).len(), 1);
}

#[test]
fn a_store_a_newer_purlis_wrote_is_never_written_over_and_nothing_is_adopted() {
    let machine = Machine::new();
    machine.folder(".claude");
    fs::create_dir_all(path(&machine.config).parent().unwrap()).unwrap();
    let newer = r#"{"version":2,"profiles":[]}"#;
    fs::write(path(&machine.config), newer).unwrap();

    let refused = machine.adoption(&[]);

    let why = refused.expect_err("refused");
    assert!(why.contains("newer purlis"), "{why}");
    assert_eq!(fs::read_to_string(path(&machine.config)).unwrap(), newer);
}

#[test]
fn a_folder_variable_that_is_not_an_absolute_path_adopts_nothing_for_its_harness() {
    let machine = Machine::new();
    machine.folder(".claude");

    let adoption = machine
        .adoption(&[("CLAUDE_CONFIG_DIR", "work/claude")])
        .unwrap();

    assert_eq!(adoption.adopted, Vec::new());
    assert_eq!(adoption.passed_over.len(), 1, "{adoption:?}");
    assert!(
        adoption.passed_over[0].contains("not an absolute path"),
        "{adoption:?}"
    );
    assert_eq!(machine.listed(), Vec::new());
}

#[test]
fn a_folder_inside_a_git_work_tree_or_a_project_is_not_adopted() {
    let machine = Machine::new();
    let repo = machine.folder("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();
    let in_repo = machine.folder("repo/claude");
    let project = machine.folder("project");
    fs::write(project.join(crate::plane::MANIFEST), "").unwrap();
    let in_project = machine.folder("project/claude");

    for folder in [in_repo, in_project] {
        let adoption = machine
            .adoption(&[("CLAUDE_CONFIG_DIR", folder.to_str().unwrap())])
            .unwrap();
        assert_eq!(adoption.adopted, Vec::new(), "{}", folder.display());
        assert!(
            adoption.passed_over[0].contains("is inside the"),
            "{adoption:?}"
        );
    }
    assert_eq!(machine.listed(), Vec::new());
}

#[cfg(unix)]
#[test]
fn a_folder_that_belongs_to_another_user_is_not_adopted() {
    if rustix::process::getuid().is_root() {
        // Every folder is root's own here, so there is no other user's to name.
        return;
    }
    let machine = Machine::new();
    // `/usr` is there on every unix this runs on, and it is root's.
    let adoption = machine.adoption(&[("CODEX_HOME", "/usr")]).unwrap();

    assert_eq!(adoption.adopted, Vec::new());
    assert!(
        adoption.passed_over[0].contains("belongs to another user"),
        "{adoption:?}"
    );
}

#[test]
fn a_folder_removed_as_a_profile_is_never_adopted_again() {
    let machine = Machine::new();
    let claude = machine.folder(".claude");
    machine.upgrade(&[]);
    update(&machine.config, |store| {
        store.profiles.retain(|p| p.name != "claude");
        store.forget(&claude);
    })
    .unwrap();

    let again = machine.upgrade(&[]);

    assert_eq!(again, Vec::new());
    assert_eq!(machine.listed(), Vec::new());
}

#[test]
fn a_field_this_purlis_does_not_know_survives_a_change_to_its_record() {
    let machine = Machine::new();
    fs::create_dir_all(path(&machine.config).parent().unwrap()).unwrap();
    fs::write(
        path(&machine.config),
        r#"{"version":1,"profiles":[{"name":"work","harness":"claude","title":"Work","folders":{"config":"/w"},"kept_logins":{"at":5}}],"later":true}"#,
    )
    .unwrap();

    update(&machine.config, |store| {
        store.profiles[0].title = "Work (this Mac)".to_owned()
    })
    .unwrap();

    let text = fs::read_to_string(path(&machine.config)).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(doc["profiles"][0]["kept_logins"]["at"], 5, "{text}");
    assert_eq!(doc["profiles"][0]["title"], "Work (this Mac)");
    assert_eq!(doc["later"], true, "{text}");
}

/// Every file under `dir` with its bytes and its modification time.
fn tree(dir: &Path) -> Vec<(PathBuf, Vec<u8>, std::time::SystemTime)> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(at) = todo.pop() {
        for entry in fs::read_dir(&at).unwrap() {
            let entry = entry.unwrap().path();
            let meta = fs::metadata(&entry).unwrap();
            if meta.is_dir() {
                todo.push(entry);
            } else {
                out.push((
                    entry.clone(),
                    fs::read(&entry).unwrap(),
                    meta.modified().unwrap(),
                ));
            }
        }
    }
    out.sort();
    out
}
