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
//! undeclared host or a model provider's through its proxy, and any address past the proxy;
//! starting a program outside what it reads, by its path, a link, a shell or as an interpreter.
//! Each is refused; what it may do (read its own folder, write its state, reach its declared host)
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
use purlis_core::skills::studio::{Kept, ListLints, ListTime, PanelKey, Studio, Templates};
use purlis_core::skills::{Template, TemplateId};
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
        Self::assembled().installed()
    }

    /// [`Self::approved`], once `laid` has added to the probe's folder what a test puts there.
    fn approved_after(laid: impl FnOnce(&Probe)) -> Self {
        let probe = Self::assembled();
        laid(&probe);
        probe.installed()
    }

    /// Assembled with its program the file `script`, as an author writes one, then approved.
    fn approved_running(script: &str) -> Self {
        use std::os::unix::fs::PermissionsExt as _;
        let probe = Self::assembled();
        let at = probe.ext().join("bin/answers");
        std::fs::write(&at, script).expect("a program");
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755)).expect("its mode");
        let manifest = probe.ext().join(extension::MANIFEST);
        let mut doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&manifest).expect("the manifest"))
                .expect("JSON");
        doc["contributes"]["runs"] = "bin/answers".into();
        std::fs::write(&manifest, doc.to_string()).expect("written");
        probe.installed()
    }

    fn installed(self) -> Self {
        let probe = self;
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
        let said = String::from_utf8_lossy(&ran.stdout).trim().to_owned();
        if said.is_empty() {
            // A probe that printed nothing did not run: say how it ended, for the reader.
            return Ok(format!(
                "(nothing; status {}, stderr {:?})",
                ran.status,
                String::from_utf8_lossy(&ran.stderr)
            ));
        }
        Ok(said)
    }

    /// What the probe saw trying `words` from inside its sandbox.
    fn tried(&self, words: &[&str]) -> String {
        self.asked(words)
            .unwrap_or_else(|why| panic!("purlis did not run the probe's try: {why}"))
    }

    /// What the probe saw trying `words`, asked again while `settled` says the answer is not
    /// yet one the network let through: the proxy looks each name up itself, and a runner's
    /// resolver drops a lookup now and then (#3261). Only a positive answer is waited for, so
    /// a broken confinement still fails every try.
    fn tried_until(&self, words: &[&str], settled: impl Fn(&str) -> bool) -> String {
        let mut said = self.tried(words);
        for _ in 1..TRIES {
            if settled(&said) {
                break;
            }
            eprintln!("{words:?}: the probe said {said:?}, so it is asked again");
            std::thread::sleep(BETWEEN_TRIES);
            said = self.tried(words);
        }
        said
    }
}

/// How many times a test through the network looks a name up, or asks the probe, before it
/// says what it found, and how long apart: a runner's resolver drops a lookup now and then.
const TRIES: u32 = 5;
const BETWEEN_TRIES: std::time::Duration = std::time::Duration::from_secs(2);

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
    let offered: Vec<TemplateId> = templates.offered().into_iter().map(|it| it.id).collect();
    let probes = TemplateId::Extension {
        extension: ID.to_owned(),
        template: "probe".to_owned(),
    };
    assert_eq!(
        offered,
        [
            TemplateId::BuiltIn(Template::Blank),
            TemplateId::BuiltIn(Template::Steps),
            probes.clone()
        ]
    );
    let text = templates
        .text(
            &probes,
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
        plugin: None,
        listed: None,
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

    // The same lint as a studio list asks it (#3940), by a deadline, and kept for the next list.
    // It declares that it answers many skills at once, so a list asks it one question for all
    // of them (#4195). A test's machine may take long to start the program the first time.
    let kept: &'static std::sync::Mutex<Kept> =
        Box::leak(Box::new(std::sync::Mutex::new(Kept::new())));
    let time = ListTime {
        time: std::time::Duration::from_secs(30),
        deadline: std::time::Duration::from_secs(30),
        now: std::time::Instant::now,
    };
    let skill = |name: &'static str| purlis_skills::SkillFiles {
        name,
        text: "---\nname: deploy\ndescription: Deploys the service when asked to.\n---\n",
        others: &others,
        origin: purlis_skills::Origin::Project,
        plugin: None,
        listed: None,
    };
    let in_one = |name: &str| {
        vec![Lint::Said {
            by: "Probe lint".to_owned(),
            says: format!("the probe was handed 2 files of {name} in one question of 3"),
        }]
    };
    let skills = [skill("deploy"), skill("review"), skill("deploy")];
    for _ in 0..2 {
        let listed: Vec<Vec<Lint>> = ListLints::new(studio.lints(), kept, time)
            .findings_of_each(&skills)
            .into_iter()
            .map(|found| found.into_iter().map(|it| it.lint).collect())
            .collect();
        assert_eq!(
            listed,
            [in_one("deploy"), in_one("review"), in_one("deploy")],
            "one question for the three, and the second list asks nothing"
        );
    }
    // Nothing more to ask in a list that has no time left: what was kept is still said.
    let none_left = ListTime {
        time: std::time::Duration::ZERO,
        ..time
    };
    assert_eq!(
        ListLints::new(studio.lints(), kept, none_left)
            .findings(&skill("review"))
            .into_iter()
            .map(|it| it.lint)
            .collect::<Vec<_>>(),
        in_one("review")
    );
    // Warm, as a list's question is timed: one start of the confined program for the list.
    let fresh: &'static std::sync::Mutex<Kept> =
        Box::leak(Box::new(std::sync::Mutex::new(Kept::new())));
    let began = std::time::Instant::now();
    ListLints::new(studio.lints(), fresh, time).findings_of_each(&skills);
    eprintln!(
        "a confined lint answered three skills in one question in {} ms, its start included",
        began.elapsed().as_millis()
    );

    // A panel on a skill's tab, after purlis's own Runs panel.
    let panels = studio.panels(&probe.plane(), "deploy");
    let keys: Vec<PanelKey> = panels.iter().map(|it| it.key.clone()).collect();
    assert_eq!(
        keys,
        [
            PanelKey::Runs,
            PanelKey::Extension {
                extension: ID.to_owned(),
                panel: "probe".to_owned(),
            }
        ]
    );
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
        None,
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
        prompt
            .runs_as_you
            .contains("runs this extension in the sandbox a chat runs in"),
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
            .any(|it| it.starts_with("hosts it reaches: probe.invalid, example.com")),
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
    let said = probe.tried(&["read", &shown(&own)]);
    assert!(said.starts_with("ok"), "it reads its own folder: {said}");
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

/// A program that says it ran, in `dir`, and a copy of the probe beside it: a script and a
/// compiled program, each of which runs where nothing holds it.
fn runnable_in(dir: &Path) -> (String, String) {
    use std::os::unix::fs::PermissionsExt as _;
    let script = dir.join("script.sh");
    std::fs::write(&script, "#!/bin/sh\necho it ran\n").expect("a script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("its mode");
    let compiled = dir.join("compiled");
    std::fs::copy(env!("CARGO_BIN_EXE_studio-probe"), &compiled).expect("a program");
    let (script, compiled) = (shown(&script), shown(&compiled));
    for program in [&script, &compiled] {
        let said = studio_probe::tried(&["start".to_owned(), program.clone()]);
        assert!(
            said.contains("exit status: 0"),
            "{program} does not run where nothing holds it: {said}"
        );
    }
    (script, compiled)
}

/// `text` as a program in the probe's own folder, which purlis fingerprints.
fn own_script(probe: &Probe, name: &str, text: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let at = probe.ext().join("bin").join(name);
    std::fs::write(&at, text).expect("a script");
    std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755)).expect("its mode");
    at
}

fn it_starts_programs_from_what_it_reads() {
    let probe = Probe::assembled();
    let script = own_script(&probe, "own.sh", "#!/bin/sh\necho its own\n");
    let probe = probe.installed();
    let said = probe.tried(&["start", "/bin/sh", "-c", "echo from the shell"]);
    assert!(said.contains("exit status: 0"), "the shell started: {said}");
    assert!(said.ends_with("from the shell"), "{said}");
    // A program of the system's, started by a program it started.
    let said = probe.tried(&["start", "/usr/bin/env", "/bin/sh", "-c", "/bin/echo twice"]);
    assert!(said.ends_with("twice"), "{said}");
    // Its own program, compiled and linked against the system's libraries, by its own path.
    let own = shown(&probe.ext().join("bin/studio-probe"));
    let said = probe.tried(&["start", &own]);
    assert!(said.contains("exit status: 0"), "{own} started: {said}");
    // A script of its own, run by the system's shell.
    let said = probe.tried(&["start", &shown(&script)]);
    assert!(said.ends_with("its own"), "{said}");
}

fn it_starts_no_program_outside_what_it_reads() {
    let probe = Probe::assembled();
    let beside = probe.dir.path().join("beside");
    let (script, compiled) = runnable_in(&beside);
    let by = own_script(&probe, "by.sh", &format!("#!{compiled}\n"));
    let probe = probe.installed();
    for program in [&script, &compiled] {
        refused(&probe.tried(&["read", program]), "reading it first");
        refused(&probe.tried(&["start", program]), program);
        // Through a link it makes in the folder it writes: the file is what is judged.
        let at = shown(&probe.ext().join("state/pointed"));
        refused(&probe.tried(&["start-linked", program, &at]), program);
        // By a shell it may start.
        let said = probe.tried(&["start", "/bin/sh", "-c", program]);
        assert!(
            said.contains("ok, it ended") && !said.contains("exit status: 0"),
            "the shell started {program}: {said}"
        );
    }
    // As the interpreter of a script it may start.
    let said = studio_probe::tried(&["start".to_owned(), shown(&by)]);
    assert!(said.contains("exit status: 0"), "unheld: {said}");
    refused(&probe.tried(&["start", &shown(&by)]), "its interpreter");
    if cfg!(target_os = "linux") {
        refused(
            &probe.tried(&["start-held", &compiled]),
            "a path-only descriptor of it",
        );
        // Handed to the system's loader as a file to run.
        for loader in ["/lib64/ld-linux-x86-64.so.2", "/lib/ld-linux-aarch64.so.1"] {
            if Path::new(loader).exists() {
                let said = probe.tried(&["start", loader, &compiled]);
                assert!(
                    said.contains("ok, it ended") && !said.contains("exit status: 0"),
                    "the loader ran it: {said}"
                );
            }
        }
    }
}

/// The probe's template, asked through the studio's own interface: the program answered.
fn it_answers_a_question(probe: &Probe) {
    let executor = Executor::default().with_deadline(PATIENT);
    let choices = Choices::read(&probe.plane());
    let templates = Templates {
        studio: Some(Studio {
            executor: &executor,
            config_root: &probe.config(),
            project: &choices,
        }),
    };
    let text = templates
        .text(
            &TemplateId::Extension {
                extension: ID.to_owned(),
                template: "probe".to_owned(),
            },
            "made-here",
            "Made from the probe's template, for a test.",
        )
        .expect("its template answers");
    assert!(text.contains("name: made-here"), "{text}");
}

/// A program an author writes as a script: by the shell, by `env`, and by an interpreter of a
/// package manager's or the system's, each answering as a compiled program does.
fn a_program_that_is_a_script_answers() {
    for script in [
        "#!/bin/sh\nexec \"$(dirname \"$0\")/studio-probe\"\n",
        "#!/usr/bin/env sh\nexec \"$(dirname \"$0\")/studio-probe\"\n",
    ] {
        it_answers_a_question(&Probe::approved_running(script));
    }
    let python = [
        "/opt/homebrew/bin/python3",
        "/usr/local/bin/python3",
        "/usr/bin/python3",
    ]
    .into_iter()
    .find(|it| {
        Path::new(it).exists() && (cfg!(target_os = "linux") || !it.starts_with("/usr/bin"))
    });
    let Some(python) = python else {
        may_skip("no Python is in a program folder here");
        return;
    };
    it_answers_a_question(&Probe::approved_running(&format!(
        "#!{python}\nimport json, sys\nask = json.loads(sys.stdin.readline())[\"ask\"]\n\
         print(json.dumps({{\"charter\": 4, \"text\": \"---\\nname: %s\\ndescription: %s\\n---\\n\" \
         % (ask[\"name\"], ask[\"description\"])}}))\n"
    )));
}

fn it_reaches_its_declared_host_and_no_other() {
    let probe = Probe::approved();
    // Declared: the proxy carries it, and finds no such host (`.invalid` never resolves). A
    // resolver that does not answer leaves the proxy's status line unsaid; any status line is
    // the proxy's verdict, and only a 502 says it carried the host.
    let declared = probe.tried_until(&["proxy", "probe.invalid"], |said| said.contains("HTTP/"));
    assert!(
        declared.contains("502"),
        "its declared host is carried: {declared} (asked up to {TRIES} times)"
    );
    let undeclared = probe.tried(&["proxy", "example.org"]);
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

/// Whether a test may pass over what this machine lacks (the network, a package manager's
/// folders): never on CI, whose runners have both, where a lack is a failure to say.
fn may_skip(why: &str) {
    assert!(std::env::var_os("CI").is_none(), "on CI, {why}");
    eprintln!("{why}, so it is not tried");
}

/// The addresses this machine looks `name` up to, looked up again while it finds none: what a
/// test through the network needs before it can say anything of the proxy.
fn looked_up(name: &str) -> Vec<std::net::IpAddr> {
    use std::net::ToSocketAddrs;
    for tried in 1..=TRIES {
        let found: Vec<std::net::IpAddr> = (name, 443)
            .to_socket_addrs()
            .map(|found| found.map(|at| at.ip()).collect())
            .unwrap_or_default();
        if !found.is_empty() || tried == TRIES {
            return found;
        }
        std::thread::sleep(BETWEEN_TRIES);
    }
    Vec::new()
}

/// Where CI provides the names these tests look up, so none of them waits on outside DNS.
const NAMES_FROM: &str = "the job's hosts file (.github/actions/studio-probe-names)";

fn a_declared_name_that_resolves_to_a_private_network_is_not_reached() {
    let probe = Probe::approved();
    // Names under `.test` (RFC 6761), which no outside resolver answers: CI writes them into
    // its hosts file, and a machine without those lines passes over this test.
    for (name, address) in [
        ("ten.studio-probe.test", "10.0.0.1"),
        ("cgnat.studio-probe.test", "100.64.0.1"),
    ] {
        let wanted: std::net::IpAddr = address.parse().expect("an address");
        if !looked_up(name).contains(&wanted) {
            may_skip(&format!(
                "{name} does not resolve to {address} here after {TRIES} lookups; on CI it \
                 comes from {NAMES_FROM}"
            ));
            continue;
        }
        let said = probe.tried(&["proxy", name]);
        assert!(
            said.contains("403"),
            "{name} ({address}) was reached: {said}"
        );
    }
}

fn a_tunnel_is_carried_only_where_its_hello_names_its_host() {
    let probe = Probe::approved();
    // A real TLS server is needed, and the proxy reaches nothing on this machine or a private
    // network, so this one goes out to example.com. CI pins its addresses in its hosts file
    // when the job starts, so the lookups here and in the proxy are local.
    if looked_up("example.com").is_empty() {
        may_skip(&format!(
            "example.com does not resolve here after {TRIES} lookups; on CI it comes from \
             {NAMES_FROM}"
        ));
        return;
    }
    let carried = |said: &str| said.contains(" 200 ") && !said.ends_with("; 0 bytes came back");
    let named = probe.tried_until(&["hello", "example.com", "example.com"], carried);
    assert!(
        carried(&named),
        "a hello naming the tunnel's own host was not carried: {named} (asked up to {TRIES} \
         times)"
    );
    for other in ["api.anthropic.com", "api.openai.com"] {
        let fronted = probe.tried(&["hello", "example.com", other]);
        assert!(
            fronted.contains(" 200 ") && fronted.ends_with("; 0 bytes came back"),
            "a hello naming {other} through example.com's open tunnel was not cut off: {fronted}"
        );
    }
}

fn it_reads_no_other_process_s_arguments_or_environment() {
    let probe = Probe::approved();
    let pid = std::process::id().to_string();
    let said = probe.tried(&["environ", &pid]);
    assert!(
        !said.contains(purlis_core::skills::STUDIO_ENV),
        "it read this process's environment: {said}"
    );
    // The same ask, made outside the sandbox, prints this test's own line and environment: so
    // the probe asks as it should, and the confined answer lacks them because it was refused.
    let unconfined = studio_probe::tried(&["environ".to_owned(), pid]);
    assert!(
        unconfined.contains("a_studio_extension_is_confined")
            && unconfined.contains(purlis_core::skills::STUDIO_ENV),
        "the ask did not print this test's own line: {unconfined}"
    );
}

fn it_reads_no_package_managers_configuration_or_state() {
    let probe = Probe::approved();
    let mut tried = 0;
    for folder in [
        "/opt/homebrew/etc",
        "/opt/homebrew/var",
        "/usr/local/etc",
        "/usr/local/var",
        "/home/linuxbrew/.linuxbrew/etc",
        "/home/linuxbrew/.linuxbrew/var",
    ] {
        let Some(file) = first_file(Path::new(folder), 3) else {
            continue;
        };
        tried += 1;
        refused(
            &probe.tried(&["read", &shown(&file)]),
            &format!("reading {}", file.display()),
        );
    }
    let bin = [
        "/opt/homebrew/bin/brew",
        "/usr/local/bin/brew",
        "/usr/bin/env",
    ]
    .into_iter()
    .find(|it| Path::new(it).exists())
    .expect("a program in a prefix");
    let said = probe.tried(&["read", bin]);
    assert!(said.starts_with("ok"), "it reads {bin}: {said}");
    if tried == 0 {
        may_skip("no package manager's configuration or state folder is here");
    }
}

/// The first plain file under `folder` this test can read, at most `depth` folders down.
fn first_file(folder: &Path, depth: usize) -> Option<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(folder)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    for entry in &entries {
        let plain = std::fs::symlink_metadata(entry).is_ok_and(|kind| kind.is_file());
        if plain && std::fs::File::open(entry).is_ok() {
            return Some(entry.clone());
        }
    }
    if depth == 0 {
        return None;
    }
    entries
        .iter()
        .filter(|entry| std::fs::symlink_metadata(entry).is_ok_and(|kind| kind.is_dir()))
        .find_map(|entry| first_file(entry, depth - 1))
}

// ---------------------------------------------------------------------------------------
// The runner
// ---------------------------------------------------------------------------------------

// ---------------------------------------------------------------------------------------
// A skill router (SK-15b, #3230; ADR 0102 §6)
// ---------------------------------------------------------------------------------------

/// The router's question about `names`, each a skill it may hide.
fn route_about(names: &[&str]) -> Vec<purlis_core::extension::studio::RouteSkill> {
    names
        .iter()
        .map(|name| purlis_core::extension::studio::RouteSkill {
            name: (*name).to_owned(),
            description: format!("Does {name}."),
            pinned: false,
        })
        .collect()
}

fn routed_by(executor: &Executor, probe: &Probe, names: &[&str]) -> Result<Vec<String>, String> {
    let skills = route_about(names);
    let budget = purlis_core::extension::studio::RouteBudget {
        skills: 30,
        chars: 4000,
    };
    let doc = executor.studio(
        &probe.config(),
        &Choices::read(&probe.plane()),
        ID,
        &purlis_core::extension::studio::Ask::Route {
            skills: &skills,
            budget,
            besides: budget,
        },
    )?;
    let hidden = purlis_core::extension::studio::routed_of(ID, names.len(), &doc)?;
    Ok(hidden.into_iter().map(|(name, _)| name).collect())
}

fn a_router_answers_confined_and_is_stopped_at_its_deadline() {
    let probe = Probe::approved();
    // A program copied in fresh is assessed by macOS the first time it runs: asked once with
    // time for that, and timed for the record.
    let patient = Executor::default()
        .with_deadline(PATIENT)
        .with_router_deadline(PATIENT);
    routed_by(&patient, &probe, &["warm"]).expect("the probe's router answers");
    let began = std::time::Instant::now();
    let hidden = routed_by(&patient, &probe, &["deploy", "probe-hide-lint", "review"])
        .expect("the probe's router answers");
    eprintln!(
        "a confined router answered in {} ms, its start included",
        began.elapsed().as_millis()
    );
    assert_eq!(hidden, ["probe-hide-lint"]);

    // One that takes two seconds is stopped at the router's own deadline, never the five a
    // view has.
    let real = Executor::default();
    let began = std::time::Instant::now();
    let refused =
        routed_by(&real, &probe, &["probe-slow", "probe-hide-x"]).expect_err("past its deadline");
    let took = began.elapsed();
    assert!(
        took < std::time::Duration::from_millis(1500),
        "a router two seconds slow held its question {took:?}"
    );
    assert!(refused.contains("did not answer within"), "{refused}");
    assert!(real.running().is_empty(), "its program was stopped");
}

// ---------------------------------------------------------------------------------------
// The Grader: its chats run through purlis, never a harness or a model's host (#3236)
// ---------------------------------------------------------------------------------------

/// A host for a grade's chats as purlis's runner is one: each chat it is asked for is "run", the
/// skill starting where the arm has it and the query should start it, and remembered.
#[derive(Default)]
struct Chats {
    asked: Vec<purlis_skills::grade::RunChat>,
    stop_after: Option<usize>,
}

impl purlis_skills::grade::Chats for Chats {
    fn run(
        &mut self,
        chat: &purlis_skills::grade::RunChat,
        _tokens_left: Option<u64>,
    ) -> purlis_skills::grade::ChatRan {
        let n = self.asked.len();
        self.asked.push(chat.clone());
        purlis_skills::grade::ChatRan {
            chat: Some(format!("purlis-chat-{n}")),
            started: if chat.arm == purlis_skills::grade::Arm::With
                && chat.prompt.starts_with("probe")
            {
                vec!["purlis-project:probe-skill".into()]
            } else {
                Vec::new()
            },
            tokens: Some(100),
            cost_usd: None,
            ms: 10,
            end: purlis_skills::grade::ChatEnd::Turn,
        }
    }

    fn cancelled(&self) -> bool {
        self.stop_after
            .is_some_and(|after| self.asked.len() >= after)
    }
}

/// A grade of `skill` with two trigger tests in two arms.
fn grade_of(skill: &str) -> purlis_skills::grade::GradeRequest {
    use purlis_skills::grade::{Arm, GradeRequest, TriggerTest};
    GradeRequest {
        skill: skill.into(),
        triggers: vec![
            TriggerTest {
                query: "probe the thing".into(),
                should_trigger: true,
            },
            TriggerTest {
                query: "what is a probe".into(),
                should_trigger: false,
            },
        ],
        outcomes: Vec::new(),
        arms: vec![Arm::With, Arm::Without],
        runs: purlis_skills::grade::RUNS,
    }
}

fn capped(tokens: u64) -> Option<purlis_skills::grade::Limits> {
    Some(purlis_skills::grade::Limits {
        tokens,
        cost_usd: None,
        priced: None,
        unpriced_usd_per_million: None,
    })
}

/// The probe graded `request`, its chats run by `chats` and held to `limits`.
fn graded_by_the_probe(
    probe: &Probe,
    request: &purlis_skills::grade::GradeRequest,
    chats: &mut Chats,
    limits: Option<purlis_skills::grade::Limits>,
) -> (purlis_skills::grade::GradeReport, Executor) {
    let executor = Executor::default().with_deadline(PATIENT);
    let choices = Choices::read(&probe.plane());
    let studio = Studio {
        executor: &executor,
        config_root: &probe.config(),
        project: &choices,
    };
    // Listed beside purlis's own, by the title its manifest gives it.
    let listed = studio.graders();
    assert_eq!(
        listed
            .iter()
            .map(|one| (one.id.as_str(), one.title.as_str()))
            .collect::<Vec<_>>(),
        [(ID, "Probe grader")]
    );
    let report = studio.grade(ID, request, chats, limits);
    (report, executor)
}

fn a_grader_runs_its_chats_through_purlis_and_never_a_harness_or_a_model() {
    use purlis_skills::grade::{Arm, GradeEnd};
    // A harness it is named, which writes a mark where it ran: in a folder nothing hands it.
    let probe = Probe::approved_after(|probe| {
        let harness = probe.dir.path().join("beside").join("claude");
        let mark = probe.dir.path().join("beside").join("harness-ran");
        std::fs::write(
            &harness,
            format!("#!/bin/sh\ntouch '{}'\necho harness\n", mark.display()),
        )
        .expect("a harness");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&harness, std::fs::Permissions::from_mode(0o755))
            .expect("executable");
        std::fs::write(
            probe.ext().join(studio_probe::HARNESS),
            harness.display().to_string(),
        )
        .expect("named");
    });
    let request = grade_of("probe-skill");
    let mut chats = Chats::default();
    let (report, executor) = graded_by_the_probe(&probe, &request, &mut chats, capped(100_000));
    assert!(executor.running().is_empty(), "its program was stopped");

    assert_eq!(report.failed, None, "{report:?}");
    assert_eq!(report.ended, GradeEnd::Whole);
    assert_eq!(report.grader, ID, "the Grader's own report");
    // Every chat it asked for was one purlis ran: the plan's, each test in each arm, each run.
    assert_eq!(
        chats.asked.len(),
        2 * 2 * usize::from(purlis_skills::grade::RUNS)
    );
    assert_eq!(report.spent_tokens, Some(100 * chats.asked.len() as u64));
    let with = report
        .triggers
        .iter()
        .find(|test| test.arm == Arm::With && test.should_trigger)
        .expect("scored");
    assert_eq!((with.runs, with.triggered, with.pass), (3, 3, Some(true)));
    assert!(
        with.chats.iter().all(|chat| chat
            .as_deref()
            .is_some_and(|chat| chat.starts_with("purlis-chat-"))),
        "each chat is one purlis ran: {:?}",
        with.chats
    );

    // And it could reach neither a model nor a harness itself.
    let tried = std::fs::read_to_string(probe.ext().join("state").join(studio_probe::GRADER_TRIED))
        .expect("the Grader said what it tried");
    eprintln!("the Grader tried:\n{tried}");
    let line = |what: &str| {
        tried
            .lines()
            .find(|line| line.starts_with(what))
            .unwrap_or_else(|| panic!("the Grader did not try {what}: {tried}"))
            .to_owned()
    };
    assert!(line("model host:").contains("403"), "{tried}");
    refused(
        line("model address:").trim_start_matches("model address: "),
        "a model's address past its proxy",
    );
    refused(
        line("harness:").trim_start_matches("harness: "),
        "a harness it started itself",
    );
    assert!(
        !probe.dir.path().join("beside").join("harness-ran").exists(),
        "the harness ran"
    );
}

fn a_grader_past_its_plan_ends_the_grade_with_a_sentence() {
    use purlis_skills::grade::GradeEnd;
    let probe = Probe::approved();
    let request = grade_of(studio_probe::PAST_THE_PLAN);
    let mut chats = Chats::default();
    let (report, executor) = graded_by_the_probe(&probe, &request, &mut chats, capped(100_000));
    assert!(executor.running().is_empty(), "its program was stopped");
    assert_eq!(report.ended, GradeEnd::GraderFailed);
    let failed = report.failed.as_deref().expect("a sentence");
    assert!(failed.contains("more runs"), "{failed}");
    assert_eq!(
        chats.asked.len(),
        2 * 2 * usize::from(purlis_skills::grade::RUNS),
        "the plan's chats ran, and none past it"
    );
}

fn a_grader_is_held_to_the_cap_and_answers_its_report_at_it() {
    use purlis_skills::grade::GradeEnd;
    let probe = Probe::approved();
    let request = grade_of("probe-skill");
    let mut chats = Chats::default();
    // Two chats reach it: the next is written back as the cap's end, and the probe reports.
    let (report, _) = graded_by_the_probe(&probe, &request, &mut chats, capped(200));
    assert_eq!(chats.asked.len(), 2, "no chat ran past the cap");
    assert_eq!(report.ended, GradeEnd::OverCap, "{report:?}");
    assert_eq!(report.failed, None);
    assert_eq!(report.grader, ID);
    assert_eq!(report.spent_tokens, Some(200));
}

fn a_stopped_grade_stops_its_grader() {
    use purlis_skills::grade::GradeEnd;
    let probe = Probe::approved();
    let request = grade_of("probe-skill");
    let mut chats = Chats {
        stop_after: Some(1),
        ..Chats::default()
    };
    let began = std::time::Instant::now();
    let (report, executor) = graded_by_the_probe(&probe, &request, &mut chats, capped(100_000));
    assert!(executor.running().is_empty(), "its program was stopped");
    assert!(
        began.elapsed() < PATIENT,
        "the Stop did not wait for the Grader"
    );
    assert_eq!(report.ended, GradeEnd::Cancelled);
    assert_eq!(chats.asked.len(), 1);
}

const TESTS: &[(&str, fn())] = &[
    (
        "a_grader_runs_its_chats_through_purlis_and_never_a_harness_or_a_model",
        a_grader_runs_its_chats_through_purlis_and_never_a_harness_or_a_model,
    ),
    (
        "a_grader_past_its_plan_ends_the_grade_with_a_sentence",
        a_grader_past_its_plan_ends_the_grade_with_a_sentence,
    ),
    (
        "a_grader_is_held_to_the_cap_and_answers_its_report_at_it",
        a_grader_is_held_to_the_cap_and_answers_its_report_at_it,
    ),
    (
        "a_stopped_grade_stops_its_grader",
        a_stopped_grade_stops_its_grader,
    ),
    (
        "a_router_answers_confined_and_is_stopped_at_its_deadline",
        a_router_answers_confined_and_is_stopped_at_its_deadline,
    ),
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
        "it_starts_programs_from_what_it_reads",
        it_starts_programs_from_what_it_reads,
    ),
    (
        "it_starts_no_program_outside_what_it_reads",
        it_starts_no_program_outside_what_it_reads,
    ),
    (
        "a_program_that_is_a_script_answers",
        a_program_that_is_a_script_answers,
    ),
    (
        "it_reaches_its_declared_host_and_no_other",
        it_reaches_its_declared_host_and_no_other,
    ),
    ("it_reaches_no_model_provider", it_reaches_no_model_provider),
    (
        "a_declared_name_that_resolves_to_a_private_network_is_not_reached",
        a_declared_name_that_resolves_to_a_private_network_is_not_reached,
    ),
    (
        "a_tunnel_is_carried_only_where_its_hello_names_its_host",
        a_tunnel_is_carried_only_where_its_hello_names_its_host,
    ),
    (
        "it_reads_no_other_process_s_arguments_or_environment",
        it_reads_no_other_process_s_arguments_or_environment,
    ),
    (
        "it_reads_no_package_managers_configuration_or_state",
        it_reads_no_package_managers_configuration_or_state,
    ),
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
