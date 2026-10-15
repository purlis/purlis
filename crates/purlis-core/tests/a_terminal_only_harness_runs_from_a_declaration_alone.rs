// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! FD-14's acceptance: **a new terminal-only harness runs from a declaration alone** (ADR 0073
//! §4). A project's `harnesses/<name>.toml` is the whole of it: no release of charter, no
//! profile, no adapter. What it gets is level 1, its terminal, and what stands between the
//! file and the program is the operator's approval on this machine, given once and asked
//! again after any change (V24b).
//!
//! The program these declarations name is `true`, because a declaration names a bare program
//! found on `PATH` and `true` is on every machine a test runs on. Nothing here runs it: a start
//! is everything up to the exec.

use std::fs;
use std::path::Path;

use purlis_core::harness::SessionId;
use purlis_core::harness_declaration;
use purlis_core::profiles::{self, Source};
use purlis_core::profiletrust;
use purlis_core::start::{self, Start};

const SHELLY: &str = r#"
name = "shelly"
title = "Shelly"
program = "true"
env = ["SHELLY_*"]

[session]
chosen_by = "purlis"
new = ["--session={id}"]
resume = ["--resume={id}"]
named_by = ["--resume"]

[capabilities]
resumes_by_id = "yes"
"#;

struct Project {
    dir: tempfile::TempDir,
}

impl Project {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("charter.toml"), "").unwrap();
        Self { dir }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn declares(&self, name: &str, text: &str) -> &Self {
        let dir = self.root().join(harness_declaration::DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("{name}.toml")), text).unwrap();
        self
    }

    /// What the picker's approve does: records the operator's yes to the profile as shown.
    fn approve(&self, name: &str) {
        let set = profiles::current(self.root());
        let p = set.get(name).expect("a profile to approve");
        let shown = profiletrust::shown(self.root(), p);
        profiletrust::approve(self.root(), p, &shown).expect("the approval is recorded");
    }

    fn start(&self, name: &str) -> Start {
        Start {
            profile: Some(name.to_owned()),
            persona: None,
            name: "ide.7".to_owned(),
            cwd: Some(self.root().to_path_buf()),
            resume: None,
            show_footer: false,
            resuming: None,
            without_sandbox: None,
            held: None,
            grants: Default::default(),
            made: purlis_core::sandbox::Made::Window,
        }
    }
}

#[test]
fn a_declared_harness_is_a_profile_named_after_it_that_runs_its_program() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.declares("shelly", SHELLY);

    let set = profiles::current(project.root());

    let shelly = set.get("shelly").expect("the declaration gave a profile");
    assert_eq!(shelly.kind, "shelly");
    assert_eq!(shelly.harness, "shelly");
    assert_eq!(shelly.command, ["true"]);
    assert_eq!(shelly.source, Source::Declared);
    // After the built-ins, which keep their places.
    let names: Vec<&str> = set.profiles().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["claude", "opencode", "codex", "shelly"]);
}

#[test]
fn a_refused_declaration_is_listed_with_the_profiles_refused_and_gives_no_profile() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.declares("shelly", "name = \"shelly\"\nprogram = \"/bin/sh\"\n");

    let set = profiles::current(project.root());

    assert!(set.get("shelly").is_none());
    let refused = set
        .refused
        .iter()
        .find(|r| r.source == "harnesses/shelly.toml")
        .expect("the refusal is listed");
    assert!(
        refused.reason.contains("not a bare program name"),
        "{refused:?}"
    );
}

#[test]
fn a_profile_in_the_local_file_may_name_a_declared_harness_as_its_kind() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.declares("shelly", SHELLY);
    fs::write(
        project.root().join(profiles::LOCAL_FILE),
        "[harness.shelly-work]\nkind = \"shelly\"\ncommand = [\"true\", \"-e\"]\n",
    )
    .unwrap();

    let set = profiles::current(project.root());

    let work = set
        .get("shelly-work")
        .unwrap_or_else(|| panic!("{:?}", set.refused));
    assert_eq!(work.kind, "shelly");
    assert_eq!(work.harness, "shelly");
    assert_eq!(work.source, Source::Local);
}

#[test]
fn a_declared_harness_is_not_started_until_the_operator_approves_it() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.declares("shelly", SHELLY);

    let refused = start::ready(&project.start("shelly"), project.root())
        .expect_err("an unapproved declaration does not start");

    assert!(
        refused.contains("harnesses/shelly.toml") && refused.contains("nobody has approved it"),
        "{refused}"
    );
}

#[test]
fn an_approved_declaration_starts_its_program_at_level_one_under_the_id_charter_chose() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.declares("shelly", SHELLY);
    project.approve("shelly");

    let ready = start::ready(&project.start("shelly"), project.root()).expect("it starts");

    assert!(ready.program.ends_with("/true"), "{}", ready.program);
    // Level 1: no adapter, so nothing arms it.
    assert_eq!(ready.harness, None);
    let id = ready.session.clone().expect("charter chose the id");
    assert_eq!(ready.args, [format!("--session={id}")]);
    assert!(
        ready
            .env
            .contains(&("PURLIS_HARNESS".to_owned(), "shelly".to_owned())),
        "{:?}",
        ready.env
    );
}

#[test]
fn an_approved_declarations_chat_carries_the_declaration_it_runs_on() {
    purlis_core::unsteered!();
    // #967: what its chat is started with (its `env`), what its pane sends for Shift+Enter and
    // how a prompt is typed into it are the declaration's, the one the launch read.
    let project = Project::new();
    project.declares(
        "shelly",
        &format!(
            "{SHELLY}\n[terminal]\nnewline = \"\\u001b\\r\"\nready_to_type = \"raw-and-quiet\"\n"
        ),
    );
    project.approve("shelly");

    let ready = start::ready(&project.start("shelly"), project.root()).expect("it starts");

    let declared = ready
        .declared
        .as_ref()
        .expect("its declaration rides the start");
    assert_eq!(declared.name, "shelly");
    assert_eq!(declared.env, ["SHELLY_*"]);
    assert_eq!(declared.terminal.newline.as_deref(), Some("\x1b\r"));
    assert_eq!(
        ready.declaration().and_then(|d| d.ready_to_type()),
        Some(purlis_core::harness::ReadyToType::WhenRawAndQuiet)
    );
}

#[test]
fn a_declared_harness_resumes_by_its_own_template() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.declares("shelly", SHELLY);
    project.approve("shelly");
    let id = SessionId::new("abc-123").unwrap();

    let ready = start::ready(
        &Start {
            resume: Some(id.clone()),
            ..project.start("shelly")
        },
        project.root(),
    )
    .expect("it starts");

    assert_eq!(ready.args, ["--resume=abc-123"]);
    assert_eq!(ready.session, Some(id));
}

#[test]
fn a_declaration_that_changed_since_it_was_approved_asks_again() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.declares("shelly", SHELLY);
    project.approve("shelly");
    project.declares("shelly", &SHELLY.replace("--session", "--yolo-session"));

    let refused = start::ready(&project.start("shelly"), project.root())
        .expect_err("a changed declaration asks again");

    assert!(refused.contains("changed"), "{refused}");
}

#[test]
fn an_approval_is_recorded_in_its_own_store_keyed_by_the_declarations_name() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.declares("shelly", SHELLY);
    project.approve("shelly");

    let record = fs::read_to_string(
        project
            .root()
            .join(".charter")
            .join(harness_declaration::APPROVED),
    )
    .expect("the store was written");
    let doc: serde_json::Value = serde_json::from_str(&record).unwrap();
    let digest = harness_declaration::read(project.root())
        .get("shelly")
        .unwrap()
        .digest
        .clone();
    assert_eq!(doc["shelly"]["digest"], digest.as_str());
    // Not the profile record: a declaration has no command of a profile's.
    assert!(
        !project
            .root()
            .join(".charter")
            .join(profiletrust::RECORD)
            .exists()
    );
}

#[test]
fn a_local_profile_on_a_declared_harness_asks_for_the_declaration_too() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.declares("shelly", SHELLY);
    fs::write(
        project.root().join(profiles::LOCAL_FILE),
        "[harness.shelly-work]\nkind = \"shelly\"\ncommand = [\"true\"]\n",
    )
    .unwrap();
    let set = profiles::current(project.root());
    let work = set.get("shelly-work").unwrap().clone();
    profiletrust::record_launched(
        project.root(),
        &work.name,
        &profiletrust::fingerprint(&work),
    )
    .unwrap();

    let refused = start::ready(&project.start("shelly-work"), project.root())
        .expect_err("the declaration was never approved");
    assert!(refused.contains("harnesses/shelly.toml"), "{refused}");

    let shown = profiletrust::shown(project.root(), &work);
    profiletrust::approve(project.root(), &work, &shown).unwrap();
    start::ready(&project.start("shelly-work"), project.root()).expect("now it starts");
}

#[test]
fn a_declared_harness_in_a_sandboxed_project_is_not_started_unconfined() {
    purlis_core::unsteered!();
    // ADR 0067 §1: fail closed. A harness with no adapter has no compiled sandbox yet.
    let project = Project::new();
    fs::write(
        project.root().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\n",
    )
    .unwrap();
    project.declares("shelly", SHELLY);
    project.approve("shelly");

    let refused = start::ready(&project.start("shelly"), project.root())
        .expect_err("a sandboxed project does not start an unconfined harness");

    assert!(refused.contains("sandbox"), "{refused}");
}

#[test]
fn a_declared_harness_in_a_project_whose_manifest_has_gone_or_is_unreadable_is_not_started() {
    purlis_core::unsteered!();
    // D-1410e: a manifest that has gone, or cannot be read, may have turned the sandbox on, so
    // a harness with no compiled sandbox is refused there too, never started unconfined.
    for (case, manifest, said) in [
        ("gone", None, "is missing"),
        (
            "unreadable",
            Some("[sandbox\nmode = "),
            "cannot be read as TOML",
        ),
    ] {
        let project = Project::new();
        project.declares("shelly", SHELLY);
        project.approve("shelly");
        match manifest {
            None => fs::remove_file(project.root().join("charter.toml")).unwrap(),
            Some(text) => fs::write(project.root().join("charter.toml"), text).unwrap(),
        }

        let refused = start::ready(&project.start("shelly"), project.root())
            .expect_err("not started unconfined");

        assert!(refused.contains(said), "{case}: {refused}");
    }
}

#[test]
fn the_approval_dialog_shows_every_word_that_will_run_and_the_whole_digest() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.declares("shelly", SHELLY);
    let set = profiles::current(project.root());
    let shelly = set.get("shelly").unwrap();
    let digest = harness_declaration::read(project.root())
        .get("shelly")
        .unwrap()
        .digest
        .clone();

    let shown = profiletrust::shown(project.root(), shelly);

    assert_eq!(
        shown,
        format!(
            "true (kind shelly, declared in harnesses/shelly.toml, {digest}; program true; a new chat adds \
             '--session={{id}}'; a resumed chat adds '--resume={{id}}'; it passes SHELLY_* from \
             your environment)"
        )
    );
    assert_eq!(digest.len(), "sha256:".len() + 64);
}

#[test]
fn the_approval_dialog_shows_the_bytes_a_declared_newline_types() {
    purlis_core::unsteered!();
    // #1119: `[terminal] newline` becomes bytes typed into the program's terminal, so the
    // operator approves it with the rest; a declaration without one shows nothing more.
    let project = Project::new();
    project.declares(
        "shelly",
        &format!("{SHELLY}\n[terminal]\nnewline = \"\\u001b\\r\"\n"),
    );
    let set = profiles::current(project.root());
    let shelly = set.get("shelly").unwrap();
    let digest = harness_declaration::read(project.root())
        .get("shelly")
        .unwrap()
        .digest
        .clone();

    let shown = profiletrust::shown(project.root(), shelly);

    assert_eq!(
        shown,
        format!(
            "true (kind shelly, declared in harnesses/shelly.toml, {digest}; program true; a new chat adds \
             '--session={{id}}'; a resumed chat adds '--resume={{id}}'; it types \"\\u001b\\u000d\" \
             for a new line; it passes SHELLY_* from your environment)"
        )
    );
}

#[test]
fn a_declaration_changed_between_the_dialog_and_the_click_is_not_approved() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.declares("shelly", SHELLY);
    let set = profiles::current(project.root());
    let shelly = set.get("shelly").unwrap().clone();
    let shown = profiletrust::shown(project.root(), &shelly);
    // Between the dialog and the click, the same program with other words.
    project.declares("shelly", &SHELLY.replace("--session", "--yolo"));

    let refused = profiletrust::approve(project.root(), &shelly, &shown)
        .expect_err("what was shown is not what is on disk");

    assert!(
        refused.contains("changed while you were reading it"),
        "{refused}"
    );
    assert!(refused.contains("--yolo"), "{refused}");
    assert!(
        !project
            .root()
            .join(".charter")
            .join(harness_declaration::APPROVED)
            .exists(),
        "nothing is recorded"
    );
    start::ready(&project.start("shelly"), project.root()).expect_err("and nothing starts");
}

#[test]
fn a_declaration_changed_between_the_check_and_the_start_runs_the_words_that_were_approved() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.declares("shelly", SHELLY);
    project.approve("shelly");
    // The launch reads once; a writer then races it.
    let read = harness_declaration::read(project.root());
    project.declares("shelly", &SHELLY.replace("--session", "--yolo"));

    let ready = start::ready_in(&project.start("shelly"), project.root(), &read)
        .expect("the approved declaration starts");

    assert!(ready.args[0].starts_with("--session="), "{:?}", ready.args);
    // And a launch that reads after the change is refused.
    let refused = start::ready(&project.start("shelly"), project.root())
        .expect_err("the changed declaration was never approved");
    assert!(refused.contains("changed"), "{refused}");
}

#[test]
fn the_dialog_warns_beside_a_word_that_names_a_file_or_folder_in_the_project() {
    purlis_core::unsteered!();
    // Ruling of 2026-10-03: a word or value naming a file in the project is a residual the
    // approval covers, so the dialog says so beside it.
    let project = Project::new();
    project.declares(
        "shelly",
        "name = \"shelly\"\nprogram = \"true\"\n[session]\nchosen_by = \"charter\"\nnew = [\"--session={id}\", \"--cfg=rel.toml\", \"run\", \"--mode=fast\"]\n",
    );
    fs::write(project.root().join("rel.toml"), "x = 1\n").unwrap();
    fs::create_dir_all(project.root().join("run")).unwrap();
    let set = profiles::current(project.root());

    let shown = profiletrust::shown(project.root(), set.get("shelly").unwrap());

    assert!(
        shown.contains(
            "--cfg=rel.toml names the file rel.toml in this project; the program may read it"
        ),
        "{shown}"
    );
    assert!(
        shown.contains("run names the folder run in this project; the program may read it"),
        "{shown}"
    );
    assert!(!shown.contains("fast names"), "{shown}");
}

#[test]
fn a_declared_word_holding_a_space_never_reads_as_two_words() {
    purlis_core::unsteered!();
    // #1014: `["--a b"]` and `["--a", "b"]` run differently, so the approval says them
    // differently. A project's declaration refuses a word with a space today; the line
    // quotes anyway, so it never depends on that rule to read one way.
    let dir = tempfile::tempdir().unwrap();
    let harnesses = dir.path().join(harness_declaration::DIR);
    fs::create_dir_all(&harnesses).unwrap();
    fs::write(harnesses.join("shelly.toml"), SHELLY).unwrap();
    let declared = harness_declaration::read(dir.path());
    assert_eq!(declared.refused, Vec::new());
    let shelly = profiles::Profile {
        name: "shelly".into(),
        kind: "shelly".into(),
        harness: "shelly".into(),
        command: vec!["true".into()],
        env: Vec::new(),
        source: Source::Declared,
    };
    let shown = |new: &[&str]| {
        let mut declared = declared.clone();
        let d = declared
            .declared
            .iter_mut()
            .find(|d| d.name == "shelly")
            .expect("shelly is declared");
        d.session.new = new.iter().map(|w| (*w).to_owned()).collect();
        profiletrust::shown_in(dir.path(), &shelly, &declared)
    };

    let one = shown(&["--a b"]);
    let two = shown(&["--a", "b"]);

    assert!(one.contains("; a new chat adds '--a b';"), "{one}");
    assert!(two.contains("; a new chat adds --a b;"), "{two}");
}

#[test]
fn the_approval_dialog_names_every_variable_the_declaration_would_pass() {
    purlis_core::unsteered!();
    // #969: the `env` namespace decides what reaches the chat from the operator's environment,
    // so the operator approves it with the words; a declaration with none says nothing more.
    let dir = tempfile::tempdir().unwrap();
    let harnesses = dir.path().join(harness_declaration::DIR);
    fs::create_dir_all(&harnesses).unwrap();
    fs::write(harnesses.join("shelly.toml"), SHELLY).unwrap();
    let declared = harness_declaration::read(dir.path());
    assert_eq!(declared.refused, Vec::new());
    let shelly = profiles::Profile {
        name: "shelly".into(),
        kind: "shelly".into(),
        harness: "shelly".into(),
        command: vec!["true".into()],
        env: Vec::new(),
        source: Source::Declared,
    };
    let shown = |env: &[&str]| {
        let mut declared = declared.clone();
        let d = declared
            .declared
            .iter_mut()
            .find(|d| d.name == "shelly")
            .expect("shelly is declared");
        d.env = env.iter().map(|w| (*w).to_owned()).collect();
        profiletrust::shown_in(dir.path(), &shelly, &declared)
    };

    let two = shown(&["SHELLY_*", "SHELLY_HOME_DIR"]);
    let none = shown(&[]);

    assert!(
        two.ends_with(
            "; a resumed chat adds '--resume={id}'; it passes SHELLY_* SHELLY_HOME_DIR from your \
             environment)"
        ),
        "{two}"
    );
    assert!(
        none.ends_with("; a resumed chat adds '--resume={id}')"),
        "{none}"
    );
}

#[test]
fn a_declared_template_of_the_word_nothing_never_reads_as_an_empty_one() {
    purlis_core::unsteered!();
    // #1014: the empty case is drawn in a form quoting never produces.
    let dir = tempfile::tempdir().unwrap();
    let harnesses = dir.path().join(harness_declaration::DIR);
    fs::create_dir_all(&harnesses).unwrap();
    fs::write(harnesses.join("shelly.toml"), SHELLY).unwrap();
    let declared = harness_declaration::read(dir.path());
    let shelly = profiles::Profile {
        name: "shelly".into(),
        kind: "shelly".into(),
        harness: "shelly".into(),
        command: vec!["true".into()],
        env: Vec::new(),
        source: Source::Declared,
    };
    let shown = |new: &[&str], resume: Option<&[&str]>| {
        let mut declared = declared.clone();
        let d = declared
            .declared
            .iter_mut()
            .find(|d| d.name == "shelly")
            .expect("shelly is declared");
        d.session.new = new.iter().map(|w| (*w).to_owned()).collect();
        d.session.resume = resume.map(|r| r.iter().map(|w| (*w).to_owned()).collect());
        profiletrust::shown_in(dir.path(), &shelly, &declared)
    };

    let empty = shown(&[], None);
    let words = shown(&["nothing"], Some(&["nothing,", "it", "cannot", "resume"]));

    assert_ne!(empty, words);
    assert!(
        empty.contains(&format!(
            "a new chat adds {}; a resumed chat adds {}",
            profiletrust::NOTHING,
            profiletrust::CANNOT_RESUME
        )),
        "{empty}"
    );
    assert!(
        words.contains("a new chat adds nothing; a resumed chat adds nothing, it cannot resume"),
        "{words}"
    );
}

// --- #968: declarations in `charter.local.toml` ------------------------------------------

/// A harness this machine alone declares, in `charter.local.toml`.
const LOCAL_SHELLY: &str = r#"
[harnesses.shelly]
title = "Shelly"
program = "true"
env = ["SHELLY_*"]

[harnesses.shelly.session]
chosen_by = "purlis"
new = ["--session={id}"]
resume = ["resume", "{id}"]

[harnesses.shelly.terminal]
newline = "\n"
"#;

/// A profile that runs Claude Code as `true`, so a start needs no harness installed, and this
/// machine's replacement of Claude Code's facts.
/// A `work` profile of Claude Code's kind on a stand-in that answers `--version` as Claude Code
/// does, beside every project: a chat starts at the floor (#2071), which asks the program it
/// starts what it is and starts none the chat could have written.
fn claude_as_stand_in() -> String {
    static ONE: std::sync::OnceLock<(tempfile::TempDir, String)> = std::sync::OnceLock::new();
    let (_, said) = ONE.get_or_init(|| {
        let dir = tempfile::tempdir().expect("a folder for the stand-in");
        let bin = stand_in::claude_code(dir.path(), "claude-stand-in", "#!/bin/sh\nexit 0\n");
        let said = format!(
            "[harness.work]\nkind = \"claude\"\ncommand = [{:?}]\n",
            bin.display().to_string()
        );
        (dir, said)
    });
    said.clone()
}
const NEWER_CLAUDE: &str = r#"
[harnesses.claude]
program = "claude"

[harnesses.claude.terminal]
newline = "\u001b[13;2u"
ready_to_type = "raw-and-quiet"
"#;

impl Project {
    fn local(&self, text: &str) -> &Self {
        fs::write(self.root().join(profiles::LOCAL_FILE), text).unwrap();
        self
    }

    /// The operator's yes to the local profile `name`, as the picker records it.
    fn launched(&self, name: &str) {
        let set = profiles::current(self.root());
        let p = set.get(name).expect("a profile to approve");
        profiletrust::record_launched(self.root(), name, &profiletrust::fingerprint(p)).unwrap();
    }
}

#[test]
fn a_harness_declared_in_the_local_file_alone_runs_as_a_chat() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.local(LOCAL_SHELLY);

    // Its profile is the local file's: held to that file's ignore check and asked once, as
    // every program that file names is, in one question that shows its declaration's words.
    let set = profiles::current(project.root());
    let shelly = set
        .get("shelly")
        .unwrap_or_else(|| panic!("{:?}", set.refused));
    assert_eq!(shelly.source, Source::Local);
    assert_eq!(shelly.command, ["true"]);
    assert!(profiletrust::declaration_approval_needed(project.root(), shelly).is_some());
    let shown = profiletrust::shown(project.root(), shelly);
    assert!(
        shown.contains("a new chat adds '--session={id}'"),
        "{shown}"
    );
    let refused = start::ready(&project.start("shelly"), project.root())
        .expect_err("a program the local file names is asked about once");
    assert!(refused.contains("nobody has approved it"), "{refused}");
    project.approve("shelly");

    let ready = start::ready(&project.start("shelly"), project.root()).expect("it starts");

    assert!(ready.program.ends_with("/true"), "{}", ready.program);
    assert_eq!(ready.harness, None, "level 1: nothing arms it");
    let id = ready.session.clone().expect("purlis chose the id");
    assert_eq!(ready.args, [format!("--session={id}")]);
    let declared = ready
        .declared
        .as_ref()
        .expect("its declaration rides the start");
    assert_eq!(declared.origin, harness_declaration::Origin::Local);
    assert_eq!(declared.env, ["SHELLY_*"]);
    assert_eq!(declared.terminal.newline.as_deref(), Some("\n"));
}

/// The file is one a chat may have written, so the words a start adds from it are the ones
/// the person was shown: a word added after the yes is asked about before it runs.
#[test]
fn a_local_harness_whose_words_changed_after_the_yes_is_asked_about_again() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.local(LOCAL_SHELLY);
    project.approve("shelly");
    start::ready(&project.start("shelly"), project.root()).expect("it starts");

    project.local(&LOCAL_SHELLY.replace(
        "new = [\"--session={id}\"]",
        "new = [\"--session={id}\", \"--unattended\"]",
    ));

    let refused = start::ready(&project.start("shelly"), project.root())
        .expect_err("a changed declaration is asked about again");
    assert!(refused.contains("is changed on this machine"), "{refused}");
    assert!(refused.contains("nobody has approved it"), "{refused}");
}

#[test]
fn a_local_harness_resumes_by_its_own_template_with_the_id_handed_over_bare() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.local(LOCAL_SHELLY);
    project.approve("shelly");
    let id = SessionId::new("abc-123").unwrap();

    let ready = start::ready(
        &Start {
            resume: Some(id),
            ..project.start("shelly")
        },
        project.root(),
    )
    .expect("it starts");

    assert_eq!(ready.args, ["resume", "abc-123"]);
}

#[test]
fn a_local_replacement_of_a_built_in_changes_its_facts_and_not_its_arming() {
    purlis_core::unsteered!();
    let plain = Project::new();
    plain.local(&claude_as_stand_in());
    plain.launched("work");
    let replaced = Project::new();
    replaced.local(&format!("{}{NEWER_CLAUDE}", claude_as_stand_in()));
    replaced.launched("work");

    let was = start::ready(&plain.start("work"), plain.root()).expect("it starts");
    let now = start::ready(&replaced.start("work"), replaced.root()).expect("it starts");

    // Its facts: its terminal is the replacement's. The words purlis adds and its namespace
    // are the built-in's, as nobody is asked about a harness purlis ships.
    let id = now.session.clone().expect("purlis still chooses the id");
    assert_eq!(now.args, ["--session-id", id.as_str(), "--name", "ide.7"]);
    let declared = now
        .declared
        .as_ref()
        .expect("the replacement rides the start");
    assert_eq!(
        declared.replaces(),
        Some(purlis_core::harness::Harness::ClaudeCode)
    );
    assert_eq!(declared.env, ["CLAUDE_*"]);
    assert_eq!(
        now.declaration()
            .and_then(|d| d.terminal.newline.as_deref()),
        Some("\x1b[13;2u")
    );
    // Typed into one line only, whatever the replacement says its terminal takes.
    assert!(
        now.declaration()
            .and_then(|d| d.why_drawn_as_a_placeholder("two\nlines"))
            .is_some()
    );
    // Its arming: the harness purlis arms, what it is handed and the sandbox are unchanged.
    assert_eq!(now.harness, Some(purlis_core::harness::Harness::ClaudeCode));
    assert_eq!(now.harness, was.harness);
    assert_eq!(now.plugins, was.plugins);
    assert_eq!(now.sandbox.is_some(), was.sandbox.is_some());
    assert_eq!(now.unsandboxed, was.unsandboxed);
    assert_eq!(now.program, was.program);
    assert_eq!(now.command, was.command);
    // And without one, the facts are the built-in's.
    assert_eq!(was.declared, None);
    let id = was.session.clone().unwrap();
    assert_eq!(was.args, ["--session-id", id.as_str(), "--name", "ide.7"]);
}

#[test]
fn a_local_replacement_that_would_change_arming_leaves_the_built_in_standing() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.local(&format!(
        "{}[harnesses.claude]\nprogram = \"claude\"\n\
         [harnesses.claude.levels]\nhooks = false\n",
        claude_as_stand_in()
    ));
    project.launched("work");

    let ready = start::ready(&project.start("work"), project.root()).expect("it starts");

    assert_eq!(
        ready.harness,
        Some(purlis_core::harness::Harness::ClaudeCode)
    );
    assert_eq!(ready.declared, None);
    let set = profiles::current(project.root());
    assert!(
        set.refused
            .iter()
            .any(|r| r.source == "charter.local.toml [harnesses.claude]"
                && r.reason.contains("Remove [levels]")),
        "{:?}",
        set.refused
    );
}

#[test]
fn a_local_replacement_in_a_file_git_would_carry_changes_nothing() {
    purlis_core::unsteered!();
    let project = Project::new();
    project.local(&format!("{}{NEWER_CLAUDE}", claude_as_stand_in()));
    project.launched("work");
    let declared = harness_declaration::read(project.root());
    // The launch's own read, with the answer git gives for a file it would carry.
    let carried = profiles::IgnoreCheck {
        reason: "charter.local.toml is tracked by git".to_owned(),
        ..profiles::IgnoreCheck::default()
    };
    let launch = (profiles::current(project.root()), carried);

    let ready = start::ready_read(&project.start("work"), project.root(), &declared, &launch)
        .expect("it starts");

    assert_eq!(ready.declared, None, "the built-in's facts stand");
    let id = ready.session.clone().unwrap();
    assert_eq!(ready.args, ["--session-id", id.as_str(), "--name", "ide.7"]);
}
