// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An extension that declares a studio capability adds a source, a lint, a template and a
//! panel to the Skill studio, and is confined** (ADR 0101, #2777), proven through purlis's real
//! record and executor.
//!
//! The probe is assembled, installed and approved as an operator would. The studio asks it
//! through the same interfaces purlis's built-ins answer: a new skill's template, a lint, a panel
//! on a skill's tab, and a skill source's listing and install review. Then its `try` command,
//! asked as `purlis studio-probe try …` asks it, attempts each thing the confinement forbids from
//! inside the sandbox and prints what happened: reading the project, the config home and a folder
//! beside its own; writing anywhere but its state folder, its own program included; reaching an
//! undeclared host or a model provider's through its proxy, and any address past the proxy. Each
//! is refused; what it may do (read its own folder, write its state, reach its declared host)
//! is done, so a refusal is the sandbox's and not a broken probe.
//!
//! **Its own runner**: on Linux the executor applies the sandbox with the program it runs in
//! (`<program> _sandbox …`, as `purlis _sandbox` is), which here is this test binary, so it serves
//! the wrap before anything else. The studio's words are known only with the `skill-studio` flag
//! on, so a run without it runs itself again with it. These need a sandbox, so they run where CI
//! runs them (macOS and Linux), never inside another sandbox.

#![cfg(unix)]

use std::path::{Path, PathBuf};

use purlis_core::executor::Executor;
use purlis_core::extension::{self, project::Choices};
use purlis_core::skills::studio::{Studio, Templates};
use purlis_skills::change::SkillTemplate as _;
use purlis_skills::{Lint, SkillLint as _};

/// How long the probe is given: not the real five seconds, as the probe is copied in fresh for
/// each test and macOS assesses a program file the first time it runs (extension-probe's #422).
const PATIENT: std::time::Duration = std::time::Duration::from_secs(30);

const ID: &str = "studio-probe";

/// A project, the probe assembled beside it, a config home to record it in, and a folder
/// beside them all that nothing hands the probe.
struct Probe {
    dir: tempfile::TempDir,
}

impl Probe {
    fn assembled() -> Self {
        let dir = tempfile::tempdir().expect("a directory");
        let plane = dir.path().join("plane");
        std::fs::create_dir_all(plane.join("skills")).expect("a project");
        std::fs::write(plane.join("charter.toml"), "# the project's own file\n").expect("a file");
        std::fs::create_dir_all(dir.path().join("beside")).expect("a folder");
        std::fs::write(dir.path().join("beside/notes.txt"), "kept beside\n").expect("a file");
        let status = std::process::Command::new(env!("CARGO_BIN_EXE_studio-probe"))
            .arg("assemble")
            .arg(dir.path().join("ext"))
            .status()
            .expect("assemble runs");
        assert!(status.success(), "assemble failed: {status}");
        Self { dir }
    }

    /// Assembled, installed and approved, the project a git repository as a project is: what
    /// purlis watches while the probe answers is what git sees.
    fn approved() -> Self {
        let probe = Self::assembled();
        let init = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(probe.plane())
            .status()
            .expect("git runs");
        assert!(init.success(), "git init failed: {init}");
        let found = extension::install(&probe.config(), &extension::BuiltIn::none(), &probe.ext())
            .expect("installed");
        extension::approve(&probe.config(), found.id(), &found.path, &found.fingerprint)
            .expect("approved");
        probe
    }

    fn config(&self) -> PathBuf {
        self.dir.path().join("config")
    }

    fn ext(&self) -> PathBuf {
        self.dir.path().join("ext")
    }

    fn plane(&self) -> PathBuf {
        self.dir.path().join("plane")
    }

    /// What the probe saw trying `words` from inside its sandbox, or why purlis did not ask it.
    fn asked(&self, words: &[&str]) -> Result<String, String> {
        let executor = Executor::default().with_deadline(PATIENT);
        let words: Vec<String> = words.iter().map(|it| (*it).to_owned()).collect();
        let ran = executor.command(
            &self.config(),
            &Choices::read(&self.plane()),
            ID,
            "try",
            &words,
        )?;
        Ok(String::from_utf8_lossy(&ran.stdout).trim().to_owned())
    }

    /// What the probe saw trying `words` from inside its sandbox.
    fn tried(&self, words: &[&str]) -> String {
        self.asked(words)
            .unwrap_or_else(|why| panic!("purlis did not run the probe's try: {why}"))
    }
}

fn shown(path: &Path) -> String {
    path.display().to_string()
}

fn refused(said: &str, what: &str) {
    assert!(
        said.starts_with("refused") && !said.contains("timed out"),
        "{what} was not refused by the sandbox: the probe said {said:?}"
    );
}

// ---------------------------------------------------------------------------------------
// The four studio capabilities, through the interfaces purlis's own answer
// ---------------------------------------------------------------------------------------

fn it_adds_a_template_a_lint_a_panel_and_a_source() {
    let probe = Probe::approved();
    let executor = Executor::default().with_deadline(PATIENT);
    let choices = Choices::read(&probe.plane());
    let studio = Studio {
        executor: &executor,
        config_root: &probe.config(),
        project: &choices,
    };

    // A template, beside purlis's own, by the extension's id.
    let templates = Templates {
        studio: Some(studio),
    };
    let offered: Vec<String> = templates.offered().into_iter().map(|it| it.id).collect();
    assert_eq!(offered, ["blank", "steps", "studio-probe/probe"]);
    let text = templates
        .text(
            "studio-probe/probe",
            "made-here",
            "Made from the probe's template, for a test.",
        )
        .expect("the probe's template answers");
    assert!(text.contains("name: made-here"), "{text}");
    assert!(text.contains("Written by the studio probe."), "{text}");

    // A lint, beside purlis's own, handed the skill's files in the question.
    let lints = studio.lints();
    assert_eq!(lints.len(), 1, "one extension lints");
    let others = vec![("scripts/run.sh".to_owned(), "#!/bin/sh\n".to_owned())];
    let findings = lints[0].findings(&purlis_skills::SkillFiles {
        name: "deploy",
        text: "---\nname: deploy\ndescription: Deploys the service when asked to.\n---\n",
        others: &others,
        origin: purlis_skills::Origin::Project,
    });
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].severity, purlis_skills::Severity::Warns);
    assert_eq!(
        findings[0].lint,
        Lint::Said {
            by: "Probe lint".to_owned(),
            says: "the probe was handed 2 files of deploy".to_owned(),
        }
    );

    // A panel on a skill's tab, after purlis's own Runs panel.
    let panels = studio.panels(&probe.plane(), "deploy");
    let keys: Vec<&str> = panels.iter().map(|it| it.key.as_str()).collect();
    assert_eq!(keys, ["runs", "ext/studio-probe/probe"]);
    let blocks = panels[1]
        .blocks
        .as_ref()
        .expect("the probe's panel answers");
    assert!(
        matches!(&blocks[1], purlis_core::panel::Block::Table(table) if table.columns == ["Handed", "Files"]),
        "{blocks:?}"
    );
    assert_eq!(panels[1].by.as_deref(), Some("Studio probe"));

    // A skill source: what it offers, and the install review purlis shows of it.
    let browse = purlis_core::skills::source::extension::browse(studio, ID).expect("it lists");
    assert_eq!(browse.revision, studio_probe::REVISION);
    let names: Vec<&str> = browse.skills.iter().map(|it| it.name.as_str()).collect();
    assert_eq!(names, [studio_probe::OFFERED]);
    let review = purlis_core::skills::source::extension::review(
        studio,
        ID,
        &[studio_probe::OFFERED.to_owned()],
    )
    .expect("purlis reviews what it answers");
    let skill = &review.skills[0];
    let files: Vec<&str> = skill.files.iter().map(|it| it.path.as_str()).collect();
    assert_eq!(files, ["SKILL.md", "scripts/check.sh"]);
    assert!(
        skill.files.iter().any(|it| it.executable && it.risky),
        "the review marks the script it would install: {:?}",
        skill.files
    );
}

fn the_approval_says_it_runs_confined_and_names_its_hosts() {
    let probe = Probe::assembled();
    let found = extension::read_at(&probe.ext()).expect("it reads");
    let prompt = extension::prompt(&found, extension::Standing::New);
    assert!(
        prompt.runs_as_you.contains("runs this extension confined"),
        "{}",
        prompt.runs_as_you
    );
    assert!(
        prompt.runs_as_you.contains("probe.invalid"),
        "{}",
        prompt.runs_as_you
    );
    assert!(
        prompt
            .declares
            .iter()
            .any(|it| it == "hosts it reaches: probe.invalid"),
        "{:?}",
        prompt.declares
    );
    assert!(
        prompt
            .declares
            .iter()
            .any(|it| it.contains("“skill-lint”, experimental")),
        "{:?}",
        prompt.declares
    );
}

fn a_manifest_naming_a_model_host_is_refused_whole() {
    let probe = Probe::assembled();
    let at = probe.ext().join(extension::MANIFEST);
    for host in [
        "api.anthropic.com",
        "api.openai.com",
        "chatgpt.com",
        "openrouter.ai",
    ] {
        let mut doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&at).expect("the manifest"))
                .expect("JSON");
        doc["hosts"] = serde_json::json!([host]);
        std::fs::write(&at, doc.to_string()).expect("written");
        let refused = extension::read_at(&probe.ext()).expect_err("a model host is refused");
        assert!(refused.contains("serves a model"), "{host}: {refused}");
    }
}

// ---------------------------------------------------------------------------------------
// The confinement, tried from inside it
// ---------------------------------------------------------------------------------------

fn it_reads_its_own_folder_and_nothing_of_the_project() {
    let probe = Probe::approved();
    let own = probe.ext().join(extension::MANIFEST);
    assert!(
        probe.tried(&["read", &shown(&own)]).starts_with("ok"),
        "it reads its own folder"
    );
    refused(
        &probe.tried(&["read", &shown(&probe.plane().join("charter.toml"))]),
        "reading the project's file",
    );
    refused(
        &probe.tried(&["read", &shown(&probe.dir.path().join("beside/notes.txt"))]),
        "reading a folder beside its own",
    );
    let record = extension::file(&probe.config());
    assert!(record.is_file(), "the record of extensions is there to try");
    refused(
        &probe.tried(&["read", &shown(&record)]),
        "reading the config home's record",
    );
}

fn it_cannot_read_the_project_through_a_link_it_makes_in_its_own_folder() {
    let probe = Probe::approved();
    let project_file = shown(&probe.plane().join("charter.toml"));
    // A hard link: the sandbox refuses making it, or reading through it.
    let at = shown(&probe.ext().join("state/borrowed"));
    let made = probe.tried(&["link", &project_file, &at]);
    let read = probe.tried(&["read", &at]);
    assert!(
        made.starts_with("refused") || read.starts_with("refused"),
        "a hard link in its state folder read the project's file: made {made:?}, read {read:?}"
    );
    // A symbolic link: reading through it is the project's file, which the sandbox refuses; and
    // purlis starts nothing more of an extension whose state folder holds one.
    let at = shown(&probe.ext().join("state/pointed"));
    let made = probe.tried(&["symlink", &project_file, &at]);
    if made == "ok" {
        let refused = probe
            .asked(&["read", &at])
            .expect_err("an extension whose state folder holds a link is not started");
        assert!(
            refused.contains("is a symlink inside the state directory"),
            "{refused}"
        );
    }
}

fn it_writes_its_state_folder_and_nothing_else() {
    let probe = Probe::approved();
    assert_eq!(
        probe.tried(&["write", &shown(&probe.ext().join("state/kept.txt"))]),
        "ok",
        "it writes its own state folder"
    );
    refused(
        &probe.tried(&["write", &shown(&probe.plane().join("planted.txt"))]),
        "writing the project",
    );
    refused(
        &probe.tried(&["write", &shown(&probe.ext().join("bin/studio-probe"))]),
        "writing its own program",
    );
    assert!(
        !probe.plane().join("planted.txt").exists(),
        "nothing was written in the project"
    );
}

fn it_reaches_its_declared_host_and_no_other() {
    let probe = Probe::approved();
    // Declared: the proxy carries it, and finds no such host (`.invalid` never resolves).
    let declared = probe.tried(&["proxy", "probe.invalid"]);
    assert!(
        declared.contains("502"),
        "its declared host is carried: {declared}"
    );
    let undeclared = probe.tried(&["proxy", "example.com"]);
    assert!(
        undeclared.contains("403"),
        "an undeclared host is refused by its proxy: {undeclared}"
    );
    // Past the proxy: no address is reached directly (TEST-NET-1, which nothing answers).
    refused(
        &probe.tried(&["connect", "192.0.2.1:443"]),
        "a connection past its proxy",
    );
}

fn it_reaches_no_model_provider() {
    let probe = Probe::approved();
    for host in [
        "api.anthropic.com",
        "api.openai.com",
        "generativelanguage.googleapis.com",
    ] {
        let said = probe.tried(&["proxy", host]);
        assert!(said.contains("403"), "{host} was reached: {said}");
    }
    // Anthropic's API address, directly.
    refused(
        &probe.tried(&["connect", "160.79.104.10:443"]),
        "a model provider's address past the proxy",
    );
}

// ---------------------------------------------------------------------------------------
// The runner
// ---------------------------------------------------------------------------------------

const TESTS: &[(&str, fn())] = &[
    (
        "it_adds_a_template_a_lint_a_panel_and_a_source",
        it_adds_a_template_a_lint_a_panel_and_a_source,
    ),
    (
        "the_approval_says_it_runs_confined_and_names_its_hosts",
        the_approval_says_it_runs_confined_and_names_its_hosts,
    ),
    (
        "a_manifest_naming_a_model_host_is_refused_whole",
        a_manifest_naming_a_model_host_is_refused_whole,
    ),
    (
        "it_reads_its_own_folder_and_nothing_of_the_project",
        it_reads_its_own_folder_and_nothing_of_the_project,
    ),
    (
        "it_cannot_read_the_project_through_a_link_it_makes_in_its_own_folder",
        it_cannot_read_the_project_through_a_link_it_makes_in_its_own_folder,
    ),
    (
        "it_writes_its_state_folder_and_nothing_else",
        it_writes_its_state_folder_and_nothing_else,
    ),
    (
        "it_reaches_its_declared_host_and_no_other",
        it_reaches_its_declared_host_and_no_other,
    ),
    ("it_reaches_no_model_provider", it_reaches_no_model_provider),
];

fn main() {
    // The wrap first: on Linux this binary is what applies the sandbox to the probe.
    if let Some(code) = purlis_linux_sandbox::serve_if_asked() {
        std::process::exit(code);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    if std::env::var(purlis_core::skills::STUDIO_ENV).as_deref() != Ok("1") {
        let me = std::env::current_exe().expect("this test");
        let status = std::process::Command::new(me)
            .args(&args)
            .env(purlis_core::skills::STUDIO_ENV, "1")
            .status()
            .expect("this test runs again with the flag on");
        std::process::exit(status.code().unwrap_or(101));
    }
    let mut filters: Vec<&str> = Vec::new();
    let (mut exact, mut list, mut ignored) = (false, false, false);
    let mut words = args.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--list" => list = true,
            "--exact" => exact = true,
            "--ignored" | "--include-ignored" | "--bench" => ignored = word != "--include-ignored",
            "--format" | "--color" | "--test-threads" | "--logfile" | "--skip" => {
                words.next();
            }
            flag if flag.starts_with('-') => {}
            filter => filters.push(filter),
        }
    }
    let chosen: Vec<&(&str, fn())> = TESTS
        .iter()
        .filter(|(name, _)| {
            !ignored
                && (filters.is_empty()
                    || filters
                        .iter()
                        .any(|it| if exact { name == it } else { name.contains(it) }))
        })
        .collect();
    if list {
        for (name, _) in &chosen {
            println!("{name}: test");
        }
        return;
    }
    let mut failed = Vec::new();
    for (name, test) in &chosen {
        let passed = std::panic::catch_unwind(test).is_ok();
        println!("test {name} ... {}", if passed { "ok" } else { "FAILED" });
        if !passed {
            failed.push(*name);
        }
    }
    println!(
        "\ntest result: {}. {} passed; {} failed",
        if failed.is_empty() { "ok" } else { "FAILED" },
        chosen.len() - failed.len(),
        failed.len()
    );
    if !failed.is_empty() {
        std::process::exit(101);
    }
}
