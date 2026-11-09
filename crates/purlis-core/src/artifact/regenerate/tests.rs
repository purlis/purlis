// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// A plane with workspace `alpha`, artifact `board`, and a skill whose script writes its data.
struct Here {
    _dir: tempfile::TempDir,
    root: PathBuf,
    workspace: PathBuf,
    config: PathBuf,
}

impl Here {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap().join("plane");
        let workspace = root.join("workspaces/alpha");
        std::fs::create_dir_all(workspace.join("artifacts/board")).unwrap();
        std::fs::create_dir_all(root.join("skills/report/scripts")).unwrap();
        std::fs::write(root.join("skills/report/SKILL.md"), "# report").unwrap();
        std::fs::write(root.join("skills/report/scripts/run.mjs"), "write()").unwrap();
        std::fs::write(root.join("skills/report/lib.mjs"), "help()").unwrap();
        Self {
            config: dir.path().canonicalize().unwrap().join("config"),
            _dir: dir,
            root,
            workspace,
        }
    }

    fn pin(&self, command: &str) -> Pin {
        Pin::new(
            "board".into(),
            "Board".into(),
            "chart".into(),
            1,
            Some(command.into()),
            None,
        )
    }

    fn resolve(&self, command: &str) -> Result<Resolved, String> {
        resolve(&self.root, &self.workspace, &self.pin(command))
    }

    fn standing(&self, command: &str) -> Standing {
        let pin = self.pin(command);
        let resolved = resolve(&self.root, &self.workspace, &pin).unwrap();
        standing(
            Some(&self.config),
            &self.workspace,
            "board",
            &pin.id,
            &resolved,
        )
    }

    fn approve(&self, command: &str) {
        let pin = self.pin(command);
        let resolved = resolve(&self.root, &self.workspace, &pin).unwrap();
        approve(&self.config, &self.workspace, "board", &pin.id, &resolved).unwrap();
    }

    fn folder(&self) -> PathBuf {
        self.workspace.join("artifacts/board")
    }
}

const COMMAND: &str = "node ../../skills/report/scripts/run.mjs --week";

fn declared(names: &[&str]) -> Declared {
    Declared {
        api: 1,
        data: names.iter().map(|n| (*n).to_owned()).collect(),
    }
}

#[test]
fn a_command_resolves_to_its_scripts_skill_folder() {
    let here = Here::new();
    let resolved = here.resolve(COMMAND).unwrap();
    assert_eq!(resolved.program, "node");
    assert_eq!(
        resolved.args,
        ["../../skills/report/scripts/run.mjs", "--week"]
    );
    assert_eq!(resolved.cwd, here.workspace);
    assert_eq!(resolved.code, here.root.join("skills/report"));
    assert_eq!(
        resolved.script,
        here.root.join("skills/report/scripts/run.mjs")
    );
}

#[test]
fn a_script_in_no_skill_is_approved_by_its_own_folder() {
    let here = Here::new();
    std::fs::create_dir_all(here.workspace.join("tools")).unwrap();
    std::fs::write(here.workspace.join("tools/make.sh"), "echo").unwrap();
    let resolved = here.resolve("./tools/make.sh").unwrap();
    assert_eq!(resolved.program, "./tools/make.sh");
    assert_eq!(resolved.code, here.workspace.join("tools"));
}

#[test]
fn a_change_to_the_resolved_code_asks_again_though_the_command_is_the_same() {
    for (file, text) in [
        ("scripts/run.mjs", "write(); steal()"),
        ("lib.mjs", "help(); steal()"),
        ("scripts/added.mjs", "new()"),
    ] {
        let here = Here::new();
        here.approve(COMMAND);
        assert_eq!(here.standing(COMMAND), Standing::Approved, "{file}");
        std::fs::write(here.root.join("skills/report").join(file), text).unwrap();
        assert_eq!(here.standing(COMMAND), Standing::Changed, "{file}");
        assert!(!Standing::Changed.runs());
    }
}

#[test]
fn a_file_removed_or_made_executable_changes_the_fingerprint() {
    use std::os::unix::fs::PermissionsExt as _;
    let here = Here::new();
    let code = here.root.join("skills/report");
    let before = fingerprint(&code).unwrap();
    assert_eq!(before, fingerprint(&code).unwrap(), "stable");
    std::fs::set_permissions(code.join("lib.mjs"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let executable = fingerprint(&code).unwrap();
    assert_ne!(before, executable);
    std::fs::remove_file(code.join("lib.mjs")).unwrap();
    assert_ne!(executable, fingerprint(&code).unwrap());
}

#[test]
fn a_link_in_the_code_is_fingerprinted_by_where_it_points_never_followed() {
    let here = Here::new();
    let code = here.root.join("skills/report");
    let outside = here.root.join("outside.mjs");
    std::fs::write(&outside, "one").unwrap();
    std::os::unix::fs::symlink(&outside, code.join("linked.mjs")).unwrap();
    let before = fingerprint(&code).unwrap();
    std::fs::write(&outside, "two").unwrap();
    assert_eq!(before, fingerprint(&code).unwrap());
    std::fs::remove_file(code.join("linked.mjs")).unwrap();
    std::os::unix::fs::symlink("/elsewhere", code.join("linked.mjs")).unwrap();
    assert_ne!(before, fingerprint(&code).unwrap());
}

#[test]
fn another_command_is_a_new_pin_and_asks_again() {
    let here = Here::new();
    here.approve(COMMAND);
    let other = "node ../../skills/report/scripts/run.mjs --month";
    assert_eq!(here.standing(other), Standing::PinChanged);
    here.approve(other);
    assert_eq!(here.standing(other), Standing::Approved);
}

#[test]
fn nothing_approved_or_no_config_home_asks() {
    let here = Here::new();
    assert_eq!(here.standing(COMMAND), Standing::New);
    let pin = here.pin(COMMAND);
    let resolved = here.resolve(COMMAND).unwrap();
    assert_eq!(
        standing(None, &here.workspace, "board", &pin.id, &resolved),
        Standing::New
    );
    std::fs::create_dir_all(crate::machine::dir(&here.config)).unwrap();
    std::fs::write(record(&here.config), "not json").unwrap();
    assert_eq!(here.standing(COMMAND), Standing::New);
}

#[test]
fn a_command_naming_no_script_is_refused() {
    let here = Here::new();
    let why = here.resolve("gh issue list --json title").unwrap_err();
    assert!(why.contains("names none"), "{why}");
    let pin = Pin::new(
        "board".into(),
        "Board".into(),
        "chart".into(),
        1,
        None,
        None,
    );
    let why = resolve(&here.root, &here.workspace, &pin).unwrap_err();
    assert!(why.contains("names no command"), "{why}");
}

#[test]
fn a_command_that_needs_a_shell_is_refused() {
    let here = Here::new();
    for command in [
        "node ../../skills/report/scripts/run.mjs > data.json",
        "node ../../skills/report/scripts/run.mjs | tee x",
        "node ../../skills/report/scripts/run.mjs && rm -rf x",
        "node '../../skills/report",
    ] {
        assert!(here.resolve(command).is_err(), "{command}");
    }
}

#[test]
fn a_working_folder_outside_the_workspace_is_refused() {
    let here = Here::new();
    let mut pin = here.pin(COMMAND);
    pin.cwd = Some("../..".into());
    let why = resolve(&here.root, &here.workspace, &pin).unwrap_err();
    assert!(why.contains("not a folder inside the workspace"), "{why}");
}

#[test]
fn place_writes_only_the_declared_files_whole() {
    let here = Here::new();
    let out = here.root.join("out");
    std::fs::create_dir_all(out.join("data")).unwrap();
    std::fs::write(out.join("data.json"), r#"{"open": 3}"#).unwrap();
    std::fs::write(out.join("data/rows.json"), "[1, 2]").unwrap();
    std::fs::write(out.join("index.html"), "<script>steal()</script>").unwrap();
    std::fs::write(out.join("data/extra.json"), "{}").unwrap();

    let placed = place(
        &here.folder(),
        &declared(&["data.json", "data/rows.json"]),
        &out,
    )
    .unwrap();
    assert_eq!(placed.written, ["data.json", "data/rows.json"]);
    assert_eq!(
        std::fs::read_to_string(here.folder().join("data.json")).unwrap(),
        r#"{"open": 3}"#
    );
    assert!(here.folder().join("data/rows.json").is_file());
    assert!(!here.folder().join("index.html").exists());
    assert!(!here.folder().join("data/extra.json").exists());
}

#[test]
fn a_failed_check_leaves_the_last_good_data_untouched() {
    let here = Here::new();
    std::fs::write(here.folder().join("data.json"), r#"{"good": true}"#).unwrap();
    let names = declared(&["data.json", "data/rows.json"]);
    let good_at = last_good(&here.folder(), &names).unwrap();

    let out = here.root.join("out");
    std::fs::create_dir_all(out.join("data")).unwrap();
    std::fs::write(out.join("data.json"), r#"{"good": false}"#).unwrap();
    std::fs::write(out.join("data/rows.json"), "not json").unwrap();
    let why = place(&here.folder(), &names, &out).unwrap_err();
    assert!(why.contains("data/rows.json is not JSON"), "{why}");
    assert_eq!(
        std::fs::read_to_string(here.folder().join("data.json")).unwrap(),
        r#"{"good": true}"#
    );
    assert_eq!(last_good(&here.folder(), &names), Some(good_at));

    let said = failed_said(&why, Some(good_at));
    assert!(said.contains("last good: "), "{said}");
    assert!(
        said.contains(&good_at.format("%Y-%m-%d").to_string()),
        "{said}"
    );
}

#[test]
fn a_run_that_wrote_nothing_declared_changes_nothing() {
    let here = Here::new();
    let out = here.root.join("out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("other.json"), "{}").unwrap();
    let why = place(&here.folder(), &declared(&["data.json"]), &out).unwrap_err();
    assert!(why.contains("wrote none of the data files"), "{why}");
    assert!(!here.folder().join("data.json").exists());
    assert_eq!(last_good(&here.folder(), &declared(&["data.json"])), None);
    assert!(failed_said(&why, None).contains("Nothing was generated"));
}

#[test]
fn a_staged_link_or_a_too_large_file_is_refused() {
    let here = Here::new();
    let out = here.root.join("out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(here.root.join("secret.json"), "{}").unwrap();
    std::os::unix::fs::symlink(here.root.join("secret.json"), out.join("data.json")).unwrap();
    let why = place(&here.folder(), &declared(&["data.json"]), &out).unwrap_err();
    assert!(why.contains("link"), "{why}");

    std::fs::remove_file(out.join("data.json")).unwrap();
    let big = format!(
        "\"{}\"",
        "x".repeat(super::super::data::MOST_BYTES as usize)
    );
    std::fs::write(out.join("data.json"), big).unwrap();
    let why = place(&here.folder(), &declared(&["data.json"]), &out).unwrap_err();
    assert!(why.contains("over the"), "{why}");
    assert!(!here.folder().join("data.json").exists());
}

#[test]
fn a_data_target_that_is_a_link_is_never_written_through() {
    let here = Here::new();
    let elsewhere = here.root.join("elsewhere.json");
    std::fs::write(&elsewhere, "{}").unwrap();
    std::os::unix::fs::symlink(&elsewhere, here.folder().join("data.json")).unwrap();
    let out = here.root.join("out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("data.json"), "[1]").unwrap();
    assert!(place(&here.folder(), &declared(&["data.json"]), &out).is_err());
    assert_eq!(std::fs::read_to_string(&elsewhere).unwrap(), "{}");
}

#[test]
fn error_output_is_its_tail_with_secrets_masked() {
    let token = format!("ghp_{}", "a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8");
    let output = format!("{}\nfailed with {token}\n", "noise ".repeat(2000));
    let shown = shown_errors(output.as_bytes());
    assert!(shown.len() <= MOST_ERROR_BYTES + 64, "{}", shown.len());
    assert!(shown.contains("failed with "), "{shown}");
    assert!(!shown.contains(&token), "{shown}");
}
