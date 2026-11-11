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
        queries: Vec::new(),
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

/// #6120, as ruled 2026-11-11: a code folder is accepted only strictly below a workspace's root
/// or in a skill's folder. Anything else is refused by one sentence, whatever shape it has.
fn refused(here: &Here, command: &str) {
    let why = here.resolve(command).unwrap_err();
    assert!(
        why.contains(
            "Regenerate approves only code below a workspace's root or in a skill's folder"
        ),
        "{command}: {why}"
    );
}

/// A file at `at` below the plane, its folders made.
fn file(here: &Here, at: &str) {
    let path = here.root.join(at);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "echo").unwrap();
}

#[test]
fn a_skill_md_at_the_plane_s_root_never_makes_it_the_code_folder() {
    let here = Here::new();
    file(&here, "workspaces/alpha/tools/make.sh");
    file(&here, SKILL_FILE);
    refused(&here, "./tools/make.sh");
}

#[test]
fn a_skill_md_at_a_workspace_s_root_never_makes_it_the_code_folder() {
    let here = Here::new();
    file(&here, "workspaces/alpha/tools/make.sh");
    file(&here, "workspaces/alpha/SKILL.md");
    refused(&here, "./tools/make.sh");
    file(&here, "workspaces/beta/tools/make.sh");
    file(&here, "workspaces/beta/SKILL.md");
    refused(&here, "/bin/sh ../beta/tools/make.sh");
}

#[test]
fn a_script_at_a_workspace_s_root_is_refused() {
    let here = Here::new();
    file(&here, "workspaces/alpha/make.sh");
    refused(&here, "./make.sh");
}

#[test]
fn a_skill_md_in_the_plane_s_workspaces_folder_never_makes_it_the_code_folder() {
    let here = Here::new();
    file(&here, "workspaces/alpha/tools/make.sh");
    file(&here, "workspaces/SKILL.md");
    refused(&here, "./tools/make.sh");
}

#[test]
fn a_script_directly_under_workspaces_is_refused() {
    let here = Here::new();
    file(&here, "workspaces/make.sh");
    refused(&here, "/bin/sh ../make.sh");
}

#[test]
fn a_skill_md_in_the_plane_s_personas_folder_never_makes_it_the_code_folder() {
    let here = Here::new();
    file(&here, "personas/steward/tools/make.sh");
    file(&here, "personas/SKILL.md");
    refused(&here, "/bin/sh ../../personas/steward/tools/make.sh");
}

#[test]
fn a_skill_md_in_the_plane_s_skills_folder_never_makes_it_the_code_folder() {
    let here = Here::new();
    file(&here, "skills/tools/make.sh");
    file(&here, "skills/SKILL.md");
    refused(&here, "/bin/sh ../../skills/tools/make.sh");
}

#[test]
fn a_script_in_no_workspace_and_no_skill_is_refused() {
    let here = Here::new();
    file(&here, "tools/make.sh");
    refused(&here, "/bin/sh ../../tools/make.sh");
    file(&here, "personas/steward/tools/make.sh");
    refused(&here, "/bin/sh ../../personas/steward/tools/make.sh");
}

#[test]
fn a_folder_under_workspaces_that_is_no_workspace_is_refused() {
    let here = Here::new();
    // A dot folder is purlis's own (`.worktrees/`), and a real clone is a repo, not a workspace.
    file(&here, "workspaces/.trash/tools/make.sh");
    refused(&here, "/bin/sh ../.trash/tools/make.sh");
    file(&here, "workspaces/repo/tools/make.sh");
    std::fs::create_dir_all(here.root.join("workspaces/repo/.git")).unwrap();
    refused(&here, "/bin/sh ../repo/tools/make.sh");
}

#[test]
fn a_folder_strictly_below_a_workspace_s_root_is_the_code_folder() {
    let here = Here::new();
    file(&here, "workspaces/alpha/tools/make.sh");
    file(&here, "workspaces/alpha/a/b/make.sh");
    file(&here, "workspaces/beta/tools/make.sh");
    let code = |command: &str| here.resolve(command).unwrap().code;
    assert_eq!(code("./tools/make.sh"), here.workspace.join("tools"));
    assert_eq!(code("./a/b/make.sh"), here.workspace.join("a/b"));
    assert_eq!(
        code("/bin/sh ../beta/tools/make.sh"),
        here.root.join("workspaces/beta/tools")
    );
    // A workspace's worktree is inside it.
    file(&here, "workspaces/alpha/.worktrees/x/tools/make.sh");
    assert_eq!(
        code("./.worktrees/x/tools/make.sh"),
        here.workspace.join(".worktrees/x/tools")
    );
}

#[test]
fn a_skill_folder_in_a_workspace_is_the_code_folder() {
    let here = Here::new();
    file(&here, "workspaces/alpha/skills/report/scripts/run.mjs");
    file(&here, "workspaces/alpha/skills/report/SKILL.md");
    assert_eq!(
        here.resolve("node skills/report/scripts/run.mjs")
            .unwrap()
            .code,
        here.workspace.join("skills/report")
    );
}

#[test]
fn an_interpreter_named_by_its_path_is_never_the_script() {
    let here = Here::new();
    std::fs::create_dir_all(here.workspace.join("tools")).unwrap();
    std::fs::write(here.workspace.join("tools/make.sh"), "echo").unwrap();
    let resolved = here.resolve("/bin/sh tools/make.sh").unwrap();
    assert_eq!(resolved.program, "/bin/sh");
    assert_eq!(resolved.code, here.workspace.join("tools"));
    let why = here.resolve("/bin/sh -c 'cat /etc/hosts'").unwrap_err();
    assert!(why.contains("names none"), "{why}");
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

/// The exploit of the review of #5978: the pin keeps the approved id while its command gains a
/// payload in words [`resolve`] passes over (`-e`), so the code folder, and the fingerprint over
/// it alone, read the same. The fingerprint covers every word, so it asks again.
#[test]
fn a_payload_added_to_the_command_under_the_approved_id_asks_again() {
    let here = Here::new();
    here.approve(COMMAND);
    let approved = here.pin(COMMAND).id;
    for forged in [
        "node -e 'steal()' ../../skills/report/scripts/run.mjs --week",
        "node --require=/tmp/x.js ../../skills/report/scripts/run.mjs --week",
        "python3 ../../skills/report/scripts/run.mjs --week",
        "node ../../skills/report/scripts/run.mjs --week --other",
    ] {
        let mut pin = here.pin(forged);
        pin.id.clone_from(&approved);
        let resolved = resolve(&here.root, &here.workspace, &pin).unwrap();
        assert_eq!(resolved.code, here.root.join("skills/report"), "{forged}");
        let standing = standing(
            Some(&here.config),
            &here.workspace,
            "board",
            &pin.id,
            &resolved,
        );
        assert_eq!(standing, Standing::Changed, "{forged}");
    }
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

fn scratch_of(here: &Here) -> Scratch {
    let at = here.root.join("scratch");
    std::fs::create_dir_all(&at).unwrap();
    Scratch::made(&at).unwrap()
}

#[test]
fn the_launch_is_the_command_wrapped_with_no_network_and_writes_to_its_scratch_alone() {
    let here = Here::new();
    let resolved = here.resolve(COMMAND).unwrap();
    let scratch = scratch_of(&here);
    let launch = launch(&here.root, &resolved, &scratch).unwrap();

    let line = [vec![launch.program.clone()], launch.args.clone()].concat();
    let tail: Vec<&str> = line.iter().rev().take(3).map(String::as_str).collect();
    assert_eq!(
        tail,
        ["--week", "../../skills/report/scripts/run.mjs", "node"],
        "{line:?}"
    );
    assert!(line.iter().any(|w| w == "/bin/sh"), "{line:?}");
    assert_eq!(launch.cwd, here.workspace);
    assert!(
        launch
            .set
            .contains(&(OUT_ENV.to_owned(), scratch.out.display().to_string()))
    );

    if cfg!(target_os = "macos") {
        let profile = &launch.args[1];
        assert!(
            !profile.contains("network-outbound"),
            "no network: {profile}"
        );
        assert!(!profile.contains(&format!("(subpath \"{}\")", here.workspace.display())));
    }
}

/// The wrapped line run for real: what it writes in its out folder is there, and a write into
/// the artifact's folder is refused by the sandbox. macOS only, where `sandbox-exec` applies the
/// profile; a test process already inside a sandbox cannot apply another, so it says so and
/// stops.
#[cfg(target_os = "macos")]
#[test]
fn a_run_writes_its_out_folder_and_never_the_artifacts() {
    let here = Here::new();
    std::fs::create_dir_all(here.workspace.join("tools")).unwrap();
    let target = here.folder().join("data.json");
    std::fs::write(
        here.workspace.join("tools/make.sh"),
        format!(
            "echo '{{\"n\": 1}}' > \"$PURLIS_ARTIFACT_OUT/data.json\"\necho bad > '{}' || echo refused >&2\n",
            target.display()
        ),
    )
    .unwrap();
    let resolved = here.resolve("/bin/sh tools/make.sh").unwrap();
    let scratch = scratch_of(&here);
    let launch = launch(&here.root, &resolved, &scratch).unwrap();
    let ran = crate::forklock::output(
        std::process::Command::new(&launch.program)
            .args(&launch.args)
            .current_dir(&launch.cwd)
            .envs(launch.set.iter().cloned()),
    )
    .unwrap();
    let said = String::from_utf8_lossy(&ran.stderr);
    if said.contains("sandbox_apply") {
        use std::io::Write as _;
        let _ = writeln!(
            std::io::stderr(),
            "skipped: this test already runs in a sandbox ({said})"
        );
        return;
    }
    assert!(ran.status.success(), "{said}");
    assert_eq!(
        std::fs::read_to_string(scratch.out.join("data.json"))
            .unwrap()
            .trim(),
        r#"{"n": 1}"#
    );
    assert!(!target.exists(), "the artifact's folder is not written");
    let log = std::fs::read_to_string(&scratch.log).unwrap();
    assert!(log.contains("refused"), "{log}");
}

/// An approval recorded before ids were 128 bits names the pin by twelve digits: under the
/// pin's id now, or handed that short id itself, it asks again and never runs.
#[test]
fn an_approval_under_an_old_short_id_asks_again_as_a_changed_pin() {
    let here = Here::new();
    let pin = here.pin(COMMAND);
    let resolved = here.resolve(COMMAND).unwrap();
    let short = &pin.id[..12];
    approve(&here.config, &here.workspace, "board", short, &resolved).unwrap();
    let standing_of =
        |id: &str| standing(Some(&here.config), &here.workspace, "board", id, &resolved);
    assert_eq!(standing_of(&pin.id), Standing::PinChanged);
    assert_eq!(standing_of(short), Standing::PinChanged);
}

/// The profile a run's line hands its sandbox, read by the reader Linux's supervisor judges
/// every open by: the one text both platforms' sandboxes hold a run to.
fn run_profile(here: &Here, command: &str) -> purlis_linux_sandbox::profile::Profile {
    let resolved = here.resolve(command).unwrap();
    let scratch = scratch_of(here);
    let machine = crate::sandbox::Machine::this();
    let launch = launch_on(
        &here.root,
        &resolved,
        &scratch,
        Some(Path::new("/opt/purlis/bin/purlis")),
        &machine,
    )
    .unwrap();
    let text = launch
        .args
        .iter()
        .find(|word| word.starts_with("(version"))
        .expect("a profile in the line");
    purlis_linux_sandbox::profile::Profile::parse(text).unwrap()
}

fn reads(path: &Path) -> purlis_linux_sandbox::profile::Op<'_> {
    purlis_linux_sandbox::profile::Op::File {
        what: purlis_linux_sandbox::profile::FileOp::ReadData,
        path,
        kind: purlis_linux_sandbox::profile::Kind::Regular,
    }
}

/// The exploit of the second review of #5978: the approved script imports `../lib/q.mjs`, a
/// file outside the code its approval fingerprints, and a teammate edits it. A run reads in the
/// plane only the approved code and the artifact's own folder, so no code it was not approved
/// for loads, nor any other file of the plane is copied into the data.
#[test]
fn a_run_reads_in_the_plane_only_its_approved_code_and_its_artifact() {
    let here = Here::new();
    std::fs::create_dir_all(here.workspace.join("lib")).unwrap();
    std::fs::write(here.workspace.join("lib/q.mjs"), "steal()").unwrap();
    std::fs::create_dir_all(here.root.join("node_modules/x")).unwrap();
    std::fs::write(here.root.join("node_modules/x/index.js"), "steal()").unwrap();
    std::fs::write(here.folder().join("data.json"), "{}").unwrap();
    let profile = run_profile(&here, COMMAND);

    for unread in [
        here.workspace.join("lib/q.mjs"),
        here.root.join("node_modules/x/index.js"),
        here.root.join("workspaces/alpha/workspace.md"),
        here.root.join("skills/other/SKILL.md"),
    ] {
        assert!(!profile.allows(&reads(&unread)), "{}", unread.display());
    }
    for read in [
        here.root.join("skills/report/scripts/run.mjs"),
        here.root.join("skills/report/lib.mjs"),
        here.folder().join("data.json"),
        PathBuf::from("/usr/lib/node/x.js"),
    ] {
        assert!(profile.allows(&reads(&read)), "{}", read.display());
    }
}

/// #6120: below the plane's root, outside the approved code and the artifact's folder, a run
/// starts no program, links no file and reads no file's extended attributes, so only code the
/// approval fingerprinted runs, however it is reached (Seatbelt judges a start and a link by
/// rules of their own, not by `file-read-data`). Linux's reader still reads the profile: it
/// sets the attributes' rule aside and leaves a filtered start to the kernel's own rule.
#[test]
fn a_run_starts_links_and_reads_attributes_of_nothing_in_the_plane_outside_its_code() {
    let here = Here::new();
    let resolved = here.resolve(COMMAND).unwrap();
    let rules = plane_reads(&here.root, &resolved).unwrap();
    let code = format!("(require-not (subpath \"{}\"))", resolved.code.display());
    let artifact = format!(
        "(require-not (subpath \"{}\"))",
        resolved.artifact.display()
    );
    let root = format!("(subpath \"{}\")", here.root.display());
    for op in [
        "file-read-data",
        "process-exec",
        "file-link",
        "file-read-xattr",
    ] {
        assert!(
            rules.iter().any(|rule| rule.starts_with("(deny ")
                && rule
                    .split(" (")
                    .next()
                    .unwrap()
                    .split(' ')
                    .any(|it| it == op)
                && rule.contains(&root)
                && rule.contains(&code)
                && rule.contains(&artifact)),
            "{op}: {rules:#?}"
        );
    }
    let profile = run_profile(&here, COMMAND);
    assert!(!profile.allows(&reads(&here.root.join("workspaces/alpha/workspace.md"))));
}

/// The same, run for real on macOS: the approved script reads its own folder and the
/// artifact's, and a read of a plane file beside them is refused by the sandbox. A test process
/// already inside a sandbox cannot apply another, so it says so and stops.
#[cfg(target_os = "macos")]
#[test]
fn a_run_is_refused_a_plane_file_outside_its_code_and_its_artifact() {
    let here = Here::new();
    std::fs::create_dir_all(here.workspace.join("tools")).unwrap();
    std::fs::create_dir_all(here.workspace.join("lib")).unwrap();
    std::fs::write(here.workspace.join("lib/q.mjs"), "secret").unwrap();
    std::fs::write(here.workspace.join("tools/own.txt"), "own").unwrap();
    std::fs::write(here.folder().join("data.json"), "{}").unwrap();
    std::fs::write(
        here.workspace.join("tools/make.sh"),
        "cat tools/own.txt artifacts/board/data.json > \"$PURLIS_ARTIFACT_OUT/seen\"\n\
         cat lib/q.mjs >> \"$PURLIS_ARTIFACT_OUT/seen\" || echo refused >&2\n",
    )
    .unwrap();
    let resolved = here.resolve("/bin/sh tools/make.sh").unwrap();
    let scratch = scratch_of(&here);
    let launch = launch(&here.root, &resolved, &scratch).unwrap();
    let ran = crate::forklock::output(
        std::process::Command::new(&launch.program)
            .args(&launch.args)
            .current_dir(&launch.cwd)
            .envs(launch.set.iter().cloned()),
    )
    .unwrap();
    let said = String::from_utf8_lossy(&ran.stderr);
    if said.contains("sandbox_apply") {
        use std::io::Write as _;
        let _ = writeln!(
            std::io::stderr(),
            "skipped: this test already runs in a sandbox ({said})"
        );
        return;
    }
    assert!(ran.status.success(), "{said}");
    let seen = std::fs::read_to_string(scratch.out.join("seen")).unwrap();
    assert_eq!(seen, "own{}", "only its code and its artifact were read");
    let log = std::fs::read_to_string(&scratch.log).unwrap();
    assert!(log.contains("refused"), "{log}");
}

#[test]
fn a_skill_folder_of_the_plane_or_of_a_persona_is_the_code_folder() {
    let here = Here::new();
    assert_eq!(
        here.resolve(COMMAND).unwrap().code,
        here.root.join("skills/report")
    );
    for level in [
        "skills.local/mine",
        "personas/steward/skills/report",
        "personas/steward/skills.local/mine",
    ] {
        file(&here, &format!("{level}/scripts/run.mjs"));
        file(&here, &format!("{level}/SKILL.md"));
        assert_eq!(
            here.resolve(&format!("node ../../{level}/scripts/run.mjs"))
                .unwrap()
                .code,
            here.root.join(level),
            "{level}"
        );
    }
}

#[test]
fn a_folder_of_a_skill_level_that_is_no_skill_is_refused() {
    let here = Here::new();
    // No `SKILL.md`: its own folder, two below the level, is no skill's.
    file(&here, "skills/notes/scripts/run.mjs");
    refused(&here, "node ../../skills/notes/scripts/run.mjs");
    file(&here, "personas/steward/skills/notes/run.mjs");
    refused(&here, "node ../../personas/steward/skills/notes/run.mjs");
    // A `SKILL.md` deeper than a skill's own folder, or in a folder that is no level.
    file(&here, "skills/notes/scripts/SKILL.md");
    refused(&here, "node ../../skills/notes/scripts/run.mjs");
    file(&here, "personas/steward/tools/run.mjs");
    file(&here, "personas/steward/tools/SKILL.md");
    refused(&here, "node ../../personas/steward/tools/run.mjs");
}

/// A folder whose name is not UTF-8, which a link can resolve a script into, is a shape the
/// allowlist cannot read: refused, never read as an empty name, which would make
/// `workspaces/<it>/…` a folder below `workspaces/` itself, and skip the dot-folder rule.
#[cfg(unix)]
#[test]
fn a_code_folder_whose_name_is_not_utf8_is_refused() {
    use std::os::unix::ffi::OsStrExt as _;
    let here = Here::new();
    let odd = std::ffi::OsStr::from_bytes(b".\xff");
    // Never made: macOS refuses such a name, and the check refuses it without reading the disk.
    let code = here.root.join("workspaces").join(odd).join("tools");
    assert!(!super::allowed(&here.root, &code));
}
