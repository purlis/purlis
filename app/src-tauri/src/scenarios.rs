// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The sandbox scenario suite** (M74.1): every scenario runs in the scenario host
//! ([`host`], T1.1-06), production's path with the shipped kit, scaled timings and the resolver
//! seam, in a child of this test binary with homes of its own.
//!
//! - [`host`]: the fixture: `Scenario::new(homes, protection, persona, dialect)`.
//! - [`switches`]: every test-only switch on a scenario's path, forced or listed.
//!
//! They bind sockets and apply purlis's OS wrap, so their red and green are CI's
//! (`sandbox-scenarios-linux`, and the `macos-scenarios`-labelled legs): a scenario that cannot
//! apply its wrap fails there and, anywhere else, says why it did not run.

#[cfg(unix)]
#[path = "scenarios/checks.rs"]
mod checks;
#[cfg(unix)]
#[path = "scenarios/claude_leg.rs"]
mod claude_leg;
#[cfg(unix)]
#[path = "scenarios/forge.rs"]
mod forge;
#[cfg(unix)]
#[path = "scenarios/host.rs"]
pub(crate) mod host;
#[cfg(unix)]
#[path = "scenarios/person.rs"]
mod person;
#[cfg(unix)]
#[path = "scenarios/queries.rs"]
mod queries;
#[cfg(unix)]
#[path = "scenarios/report.rs"]
mod report;
#[path = "scenarios/switches.rs"]
mod switches;

#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
use host::{Dialect, Homes, Protection, Scenario};

/// What a binary says it was built with (`purlis_core::fence::BUILT_WITH_ARG`).
#[cfg(unix)]
fn built_with(binary: &Path) -> String {
    let out = purlis_core::forklock::output(
        std::process::Command::new(binary).arg(purlis_core::fence::BUILT_WITH_ARG),
    )
    .unwrap_or_else(|why| panic!("{} does not run: {why}", binary.display()));
    assert!(out.status.success(), "{}: {out:?}", binary.display());
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// What a release build says it was built with.
#[cfg(unix)]
const AS_RELEASED: &str = "fenced=false test-support=false";

/// The files in `folder`, by name.
#[cfg(unix)]
fn files_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .map(|read| {
            read.flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// **The scenario kit is what the app ships** (T1.1-06): the `purlis` CI built apart from the
/// test build, with neither the fence nor the test seams, never the tests' stand-in; the repo's
/// plugin, not an empty folder; git hooks and shims that binary wrote; the skill sets' and chat
/// settings' folders; and the same kit on the window's side and as the wrap. The host process
/// `in_a_host_process_at` runs is a release build's too.
#[cfg(unix)]
#[test]
fn the_scenario_kit_is_what_the_app_ships() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(scenario) = Scenario::new(&homes, Protection::Full, None, Dialect::Codex) else {
        return;
    };
    let kit = &scenario.kit;

    let head = std::fs::read(&kit.binary).expect("the binary reads");
    assert!(
        !head.starts_with(b"#!"),
        "{} is a script, the tests' stand-in, not a built purlis",
        kit.binary.display()
    );
    assert_ne!(Some(&kit.binary), crate::Shipped::default().binary.as_ref());
    assert_eq!(
        built_with(&kit.binary),
        AS_RELEASED,
        "the purlis a chat runs"
    );
    match std::env::var_os(host::HOST_APP_VAR) {
        Some(app) => assert_eq!(
            built_with(Path::new(&app)),
            AS_RELEASED,
            "the host process `in_a_host_process_at` runs"
        ),
        None => assert!(
            !(cfg!(target_os = "linux") && host::on_ci()),
            "{} names no host process on Linux CI",
            host::HOST_APP_VAR
        ),
    }

    assert_eq!(kit.plugin, host::repo().join("app/src-tauri/plugin"));
    assert!(
        kit.plugin
            .join(".claude-plugin")
            .join("plugin.json")
            .is_file(),
        "the repo's plugin, with its manifest: {:?}",
        files_in(&kit.plugin)
    );
    for (what, folder) in [
        ("git hooks", kit.git_hooks.dir().to_path_buf()),
        ("shims", kit.shims.bin()),
    ] {
        let written = files_in(&folder);
        assert!(
            !written.is_empty(),
            "no {what} were written in {}",
            folder.display()
        );
        let names_the_binary = written.iter().any(|name| {
            std::fs::read_to_string(folder.join(name))
                .is_ok_and(|text| text.contains(&kit.binary.display().to_string()))
        });
        assert!(
            names_the_binary,
            "the {what} run another purlis: {written:?}"
        );
    }
    let for_the_host = kit.for_the_host();
    assert!(for_the_host.skill_sets.is_some(), "skill_sets is set");
    assert!(for_the_host.chat_settings.is_some(), "chat_settings is set");
    assert!(for_the_host.git_hooks.is_some(), "the git hooks are armed");

    let shipped = scenario.planes().shipped().clone();
    assert_eq!(
        shipped.binary.as_ref(),
        Some(&kit.binary),
        "the window's binary"
    );
    assert_eq!(
        shipped.plugin.as_ref(),
        Some(&kit.plugin),
        "the window's plugin"
    );
    assert!(shipped.shims.is_some(), "the window's shims");
    assert!(shipped.git_hooks.is_some(), "the window's git hooks");
    assert_eq!(
        crate::sandboxing::scenario_wrap(),
        Some(kit.binary.as_path()),
        "the wrap"
    );
}

/// **The scenario host runs chats in the session host** (T1.1-06): chats run in the host by
/// default, the window's chats are the host's, the host's own `Confine` started the chat's
/// confinement, and the host, not the window, writes the event log in the scenario's data home.
#[cfg(unix)]
#[test]
fn the_scenario_host_runs_chats_in_the_session_host() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    assert!(
        crate::hostedsessions::chats_run_in_the_host(),
        "production's default: chats run in the session host"
    );
    let Some(scenario) = Scenario::new(&homes, Protection::Full, None, Dialect::Codex) else {
        return;
    };
    let chat = scenario.starts(&serde_json::json!({
        "turns": [{"prompt": "go", "calls": [{"step": "here", "tool": "bash", "run": "echo here"}]}]
    }));
    scenario.until_played(chat, &["here"]);

    let started = scenario.confines.started();
    assert!(
        !started.is_empty(),
        "the host's own Confine started the chat's confinement"
    );
    assert!(
        started
            .iter()
            .all(|one| one.planned != purlis_host::Planned::Elsewhere),
        "{started:?}"
    );
    let events = scenario.events();
    assert!(
        !events.is_empty(),
        "the host wrote the event log in the scenario's own folder of the data home"
    );
}

/// **Every template is what a chat starts under** (T1.1-06, Q308a): in a project whose manifest
/// names it, as the scenario host writes it, `strict`, `watch`, `hold`, `team` and `open` each
/// resolve at a chat's start ([`purlis_core::enforcement::at_start`]) to exactly that template,
/// in a process that is purlis's app as `main.rs` makes it; `strict` is every family enforced,
/// never the product's default `watch`, and `open`, looser than the `watch` a person accepts
/// first, only once the person pressed. Needs no wrap, so it runs everywhere.
#[cfg(unix)]
#[test]
fn every_template_is_what_a_chat_starts_under() {
    use purlis_core::enforcement::{Mode, Resolved, at_start};
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let mut seen = Vec::new();
    for (n, template) in host::Template::ALL.into_iter().enumerate() {
        let root = homes.projects.join(format!("template-{n}"));
        std::fs::create_dir_all(&root).expect("the project");
        let root = root.canonicalize().expect("the project");
        std::fs::write(
            root.join(purlis_core::plane::MANIFEST),
            host::policy_of(template),
        )
        .expect("its manifest");
        let mut pressed = false;
        host::runs_under(template, &root, |shown| {
            pressed = true;
            purlis_core::enforcement::press(&root, &shown).expect("the person's press is kept");
        });
        assert_eq!(
            pressed,
            template == host::Template::Open,
            "{}: the person is asked to press only for a template looser than `watch`",
            template.word()
        );
        let resolved = at_start(&root);
        assert_eq!(resolved, host::resolved_under(template, &root));
        seen.push((template, resolved));
        std::fs::remove_dir_all(&root).expect("the project goes");
    }
    let of = |wanted: host::Template| {
        seen.iter()
            .find(|(template, _)| *template == wanted)
            .map(|(_, resolved)| resolved.clone())
            .expect("resolved")
    };
    assert_eq!(of(host::Template::Strict), Resolved::all(Mode::Enforce));
    assert_ne!(of(host::Template::Watch), of(host::Template::Strict));
    assert_ne!(of(host::Template::Hold), of(host::Template::Watch));
    assert_ne!(of(host::Template::Open), of(host::Template::Watch));
    homes.ran_no_scenario();
}

/// **A chat runs under the template its scenario names** (T1.1-06, Q308a): in the scenario host,
/// a chat under `strict`, `watch`, `hold`, `team` or `open` is handed that template's resolution
/// by its start (`PURLIS_POLICY`, which can only tighten what its hooks enforce by), and under
/// `strict` every family is enforced. What its hooks enforce by is the app's answer to their
/// `Ask::Policy`, `strict` where none comes, so the policy is also proven at a hook: a line
/// `guards.save` refuses (A6, a live substitution in purlis's own prose) is refused where the
/// template enforces it, and runs with one log line where it logs it.
#[cfg(unix)]
#[test]
fn a_chat_runs_under_the_template_its_scenario_names() {
    use purlis_core::enforcement::{Family, Mode};
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    for template in host::Template::ALL {
        let Some(scenario) =
            Scenario::under(&homes, Protection::Full, None, Dialect::Codex, template)
        else {
            return;
        };
        let chat = scenario.starts(&serde_json::json!({
            "turns": [{"prompt": "go", "calls": [
                {"step": "policy", "tool": "bash",
                 "run": format!("echo policy=${}", purlis_core::enforcement::ENV)},
                {"step": "saves-a-substitution", "tool": "bash", "run": SAVES_A_SUBSTITUTION}
            ]}]
        }));
        scenario.until_played(chat, &["policy", "saves-a-substitution"]);
        let resolved = host::resolved_under(template, &scenario.root);
        let expected = resolved.spelled();
        let said = stdout_of(&scenario, chat, "policy");
        assert_eq!(
            said.trim(),
            format!("policy={expected}"),
            "{}: the chat's start handed it another policy\n{}",
            template.word(),
            scenario.said()
        );
        if template == host::Template::Strict {
            assert!(
                !said.contains("=off") && !said.contains("=log"),
                "strict: a family is not enforced: {said}"
            );
        }
        // At the hook: the guard's own verdict on a line `guards.save` refuses.
        let saves = scenario
            .result_of(chat, "saves-a-substitution")
            .expect("played");
        // A refusal is the fake's `hook` and `reason` (`transcript::tool_result`); the guard's
        // A6 names the command it reads as persisting prose.
        let refused = saves["hook"] == "pretooluse";
        if refused {
            assert!(
                saves["reason"]
                    .as_str()
                    .is_some_and(|why| why.contains("persona remember") && why.contains("$(…)")),
                "{}: the hook refused the line for another reason than A6: {saves}",
                template.word()
            );
        } else {
            assert!(
                stdout_of(&scenario, chat, "saves-a-substitution").contains("let-through"),
                "{}: the line was neither refused nor run: {saves}",
                template.word()
            );
        }
        let mode = resolved.mode(Family::GuardsSave);
        assert_eq!(
            refused,
            mode == Mode::Enforce,
            "{}: guards.save is {}, but the hook {} the line: {saves}\n{}",
            template.word(),
            mode.word(),
            if refused { "refused" } else { "let through" },
            scenario.said()
        );
        if mode == Mode::Log {
            until(
                &scenario,
                &format!("{}: the hook's A6 line logged", template.word()),
                || logged_a6(&scenario.root),
            );
        } else {
            assert!(
                !logged_a6(&scenario.root),
                "{}: guards.save is {}, yet the hook logged its line",
                template.word(),
                mode.word()
            );
        }
    }
}

/// A line `guards.save` refuses (A6): a purlis command that persists prose carrying a live
/// substitution. Behind `false &&`, so where it is let through it saves nothing.
#[cfg(unix)]
const SAVES_A_SUBSTITUTION: &str =
    "false && purlis persona remember steward \"a $(echo x) b\"; echo let-through";

/// Whether any chat's enforcement log in the project at `root` has an A6 line of `guards.save`.
#[cfg(unix)]
fn logged_a6(root: &Path) -> bool {
    std::fs::read_dir(purlis_core::enforcement::log_dir(root))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .flat_map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .collect::<Vec<_>>()
        })
        .any(|line| line["rule"] == "A6" && line["family"] == "guards.save")
}

/// **One `Timings` value reaches the board, the throttle and the hold's end, in host mode**
/// (T1.1-06; T1.1-01's in-process test is `planes::tests::…_in_process`): the scaled value the
/// scenario was given is what the host's board holds an ask for, what the window's hooks throttle
/// blocks by and end a hold at, and what its chats hand their hearing.
#[cfg(unix)]
#[test]
fn one_timings_value_reaches_the_board_the_throttle_and_hold_ends_in_host_mode() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(scenario) = Scenario::new(&homes, Protection::Full, None, Dialect::Codex) else {
        return;
    };
    let timings = scenario.timings;
    assert_ne!(
        timings,
        purlis_core::sandbox::timings::Timings::production()
    );
    let chat = scenario.starts(&serde_json::json!({
        "turns": [{"prompt": "go", "calls": [{"step": "here", "tool": "bash", "run": "echo here"}]}]
    }));
    scenario.until_played(chat, &["here"]);

    let started = scenario.confines.started();
    assert!(!started.is_empty(), "a confinement was started in the host");
    for one in &started {
        assert_eq!(
            one.holds_asks_for,
            Some(timings.hold()),
            "the host's board: {one:?}"
        );
    }
    let held = scenario.held();
    assert_eq!(held.hooks().timings(), timings, "the window's hooks");
    assert_eq!(
        held.hooks().throttle_window(),
        timings.throttle_window(),
        "the block throttle"
    );
    assert_eq!(
        crate::asking::hold_ends(None, 1_000, &held.hooks().timings()),
        1_000 + timings.hold().as_secs(),
        "the window's end of a hold"
    );
    assert_eq!(held.chats().timings(), timings, "the window's chats");
}

/// **A Linux Claude Code chat in the scenario host is sandboxed** (T1.1-06): the production
/// branch of `Chats::start_as_opted`, never the test build's that can leave one unsandboxed, at
/// the floor and at Full. Its command runs under purlis's own wrap: held by no-new-privileges and a
/// seccomp filter, as it says itself, below `<kit> _sandbox`, as `/proc` says from outside. A plain file of the
/// project reads (the control), the vault no chat reads is refused, and in its config folder a
/// write where a turn writes goes through while a new file beside Claude Code's own settings is
/// refused, which only `claude_wrap` refuses at the floor. On
/// macOS Claude Code holds itself (ticket 10, #6900): reported `not_run`, never passed.
///
/// purlis's Linux wrap is Landlock and a seccomp supervisor (`purlis _sandbox`), not a user
/// namespace: the chat runs in the first user namespace by design, so it is seccomp's filter
/// mode (`Seccomp: 2`) that is asserted, not a namespace of its own.
#[cfg(unix)]
#[test]
fn a_linux_claude_chat_in_the_scenario_host_is_sandboxed() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    if !cfg!(target_os = "linux") {
        report::write(&[a_fixture_record(
            "a_linux_claude_chat_in_the_scenario_host_is_sandboxed",
            Some(purlis_scenarios::model::Dialect::Claude),
            None,
            purlis_scenarios::model::ReportVerdict::NotRun,
            "purlis's wrap holds a Claude Code chat on Linux alone (`claude_wrap::wrap`); on \
             macOS it runs Claude Code's own sandbox, which the fake does not play (ticket 10, \
             #6900)",
        )])
        .expect("the leg's report");
        homes.ran_no_scenario();
        return;
    }
    for protection in [Protection::Basic, Protection::Full] {
        let Some(scenario) = Scenario::new(&homes, protection, None, Dialect::Claude) else {
            return;
        };
        let vaults = scenario.root.join(".purlis").join("vaults");
        std::fs::create_dir_all(&vaults).expect("the vaults");
        std::fs::write(
            vaults.join("covering.json"),
            "{\"covering\": \"VAULT-HELD\"}\n",
        )
        .expect("a vault");
        std::fs::write(scenario.root.join("plain.txt"), "PLAIN-READS\n").expect("a plain file");
        let reads = format!("cat {}", vaults.join("covering.json").display());
        let chat = scenario.starts(&serde_json::json!({
            "turns": [{"prompt": "go", "calls": [
                {"step": "where", "tool": "bash", "run": WHERE_AM_I},
                {"step": "reads-plain", "tool": "bash",
                 "run": format!("cat {}", scenario.root.join("plain.txt").display())},
                {"step": "reads-the-vault", "tool": "bash", "run": reads},
                {"step": "writes-its-settings", "tool": "bash", "run": WRITES_ITS_SETTINGS}
            ]}]
        }));
        scenario.until_played(chat, &["writes-its-settings"]);
        let said = scenario.said();
        assert!(
            !scenario.confines.started().is_empty(),
            "{protection:?}: the chat runs confined, its proxy started by the host"
        );
        let wrapped = format!("{} _sandbox", scenario.kit.binary.display());
        let place = stdout_of(&scenario, chat, "where");
        // The wrap's own process is outside what the chat can read of itself, so its ancestry
        // is read from here, as the system has it. Each of the three is said, so a red run
        // names every one that does not hold.
        let ancestry = ancestors_of_harness(&scenario.homes.scripts.join("chat-1.json"), &wrapped);
        let status_says = |key: &str, value: &str| {
            place
                .lines()
                .any(|l| l.split_whitespace().collect::<Vec<_>>() == [key, value])
        };
        let mut not_held = Vec::new();
        if !status_says("NoNewPrivs:", "1") {
            not_held.push("no-new-privileges (`NoNewPrivs: 1`)".to_owned());
        }
        if !status_says("Seccomp:", "2") {
            not_held.push("a seccomp filter (`Seccomp: 2`)".to_owned());
        }
        if !ancestry.iter().any(|line| line.starts_with(&wrapped)) {
            not_held.push(format!("`{wrapped}` above the harness: {ancestry:#?}"));
        }
        assert!(
            not_held.is_empty(),
            "{protection:?}: the chat's command is not held by {not_held:#?}\n{place}\n{said}"
        );
        assert!(
            stdout_of(&scenario, chat, "reads-plain").contains("PLAIN-READS"),
            "{protection:?}: the control: a plain file of the project does not read\n{said}"
        );
        let read = scenario.result_of(chat, "reads-the-vault").expect("played");
        assert!(
            !read.to_string().contains("VAULT-HELD"),
            "{protection:?}: the Claude Code chat read the vault, so it ran unsandboxed: {read}\n{said}"
        );
        let wrote = stdout_of(&scenario, chat, "writes-its-settings");
        assert!(
            wrote.contains("config=/") && wrote.contains("exists"),
            "{protection:?}: the chat has no config folder to write in: {wrote}\n{said}"
        );
        assert!(
            wrote.contains("control=wrote"),
            "{protection:?}: the control: the chat could not write where a turn writes in its \
             config folder: {wrote}\n{said}"
        );
        assert!(
            wrote.contains("probe=refused"),
            "{protection:?}: the chat wrote beside Claude Code's own settings, which \
             `claude_wrap` keeps from it: {wrote}\n{said}"
        );
    }
}

/// **On macOS the OS wrap is on a Codex and an opencode chat's process** (T1.1-06, #7152): at
/// Full, under `strict`, each chat reads a plain file of its project (the control) and then the
/// project's vault, and only the second is refused, by Seatbelt (`Operation not permitted`). The
/// vault's path is put together in the shell, so no guard that reads a command's words refuses
/// it first: what refuses it is `sandbox-exec`, and a chat started without it reads the vault and
/// fails here. Claude Code on macOS holds itself (ticket 10, #6900), and Linux's wrap is asserted
/// by `a_linux_claude_chat_in_the_scenario_host_is_sandboxed`: both are reported `not_run`.
#[cfg(unix)]
#[test]
fn a_macos_chat_is_held_by_the_os_wrap() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    if !cfg!(target_os = "macos") {
        report::write(&[a_fixture_record(
            "a_macos_chat_is_held_by_the_os_wrap",
            None,
            Some(purlis_scenarios::model::Protection::Full),
            purlis_scenarios::model::ReportVerdict::NotRun,
            "the macOS wrap (`sandbox-exec`) alone; Linux's is \
             `a_linux_claude_chat_in_the_scenario_host_is_sandboxed`",
        )])
        .expect("the leg's report");
        homes.ran_no_scenario();
        return;
    }
    report::write(&[a_fixture_record(
        "a_macos_chat_is_held_by_the_os_wrap",
        Some(purlis_scenarios::model::Dialect::Claude),
        Some(purlis_scenarios::model::Protection::Full),
        purlis_scenarios::model::ReportVerdict::NotRun,
        "Claude Code on macOS runs its own sandbox, which the fake does not play (ticket 10, \
         #6900)",
    )])
    .expect("the leg's report");
    for dialect in [Dialect::Codex, Dialect::OpenCode] {
        let Some(scenario) = Scenario::under(
            &homes,
            Protection::Full,
            None,
            dialect,
            host::Template::Strict,
        ) else {
            return;
        };
        let vaults = scenario.root.join(".purlis").join("vaults");
        std::fs::create_dir_all(&vaults).expect("the vaults");
        std::fs::write(
            vaults.join("covering.json"),
            "{\"covering\": \"VAULT-HELD\"}\n",
        )
        .expect("a vault");
        std::fs::write(scenario.root.join("plain.txt"), "PLAIN-READS\n").expect("a plain file");
        let root = scenario.root.display();
        let chat = scenario.starts(&serde_json::json!({
            "turns": [{"prompt": "go", "calls": [
                {"step": "reads-plain", "tool": "bash",
                 "run": format!("cat '{root}/plain.txt' 2>&1; echo exit=$?")},
                {"step": "reads-the-vault", "tool": "bash",
                 "run": format!(
                     "d='{root}/.pur'; d=\"${{d}}lis\"; cat \"$d/vaults/covering.json\" 2>&1; \
                      echo exit=$?"
                 )}
            ]}]
        }));
        scenario.until_played(chat, &["reads-the-vault"]);
        let said = scenario.said();
        let plain = stdout_of(&scenario, chat, "reads-plain");
        assert!(
            plain.contains("PLAIN-READS") && plain.contains("exit=0"),
            "{dialect:?}: the control: a plain file of the project does not read: {plain}\n{said}"
        );
        let vault = stdout_of(&scenario, chat, "reads-the-vault");
        assert!(
            !vault.contains("VAULT-HELD"),
            "{dialect:?}: the chat read the vault, so no OS wrap is on its process: {vault}\n{said}"
        );
        assert!(
            vault.contains("Operation not permitted") && !vault.contains("exit=0"),
            "{dialect:?}: the vault read was not refused by Seatbelt: {vault}\n{said}"
        );
    }
}

/// **The command lines of the ancestors of the chat harness playing `script`**, read from
/// `/proc` as the system has them (Linux), from its parent up to the first process. The harness
/// is the process running the fake with `script` that is not the wrap (`wrapped`), whose own
/// command line names both; and only a strict ancestor is answered, so the wrap is never taken
/// for an ancestor of itself.
#[cfg(unix)]
fn ancestors_of_harness(script: &Path, wrapped: &str) -> Vec<String> {
    let cmdline = |pid: u32| {
        std::fs::read(format!("/proc/{pid}/cmdline"))
            .map(|raw| {
                String::from_utf8_lossy(&raw)
                    .replace('\0', " ")
                    .trim()
                    .to_owned()
            })
            .unwrap_or_default()
    };
    let parent = |pid: u32| {
        std::fs::read_to_string(format!("/proc/{pid}/status"))
            .ok()?
            .lines()
            .find_map(|line| line.strip_prefix("PPid:"))?
            .trim()
            .parse::<u32>()
            .ok()
    };
    let script = script.display().to_string();
    let harness = std::fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .find(|pid| {
            let line = cmdline(*pid);
            line.contains("fake-harness") && line.contains(&script) && !line.starts_with(wrapped)
        });
    let mut out = Vec::new();
    let mut at = harness.and_then(parent);
    while let Some(pid) = at.filter(|pid| *pid > 1 && Some(*pid) != harness) {
        out.push(cmdline(pid));
        at = parent(pid);
    }
    out
}

/// **A Claude Code chat's writes in its own config folder**, saying where, that the folder is
/// there, and whether each was refused: one where a turn writes (`projects`, the control, which
/// must be written) and a new file beside its settings, which only `claude_wrap` refuses at the
/// floor (`claude_wrap::Wrap::writable`, `HOME_DIRS`).
#[cfg(unix)]
const WRITES_ITS_SETTINGS: &str = "d=${CLAUDE_CONFIG_DIR:-$HOME/.claude}; echo config=$d; \
     [ -d \"$d\" ] && echo exists; \
     if printf probe > \"$d/projects/scenario-probe\" 2>/dev/null; then echo control=wrote; \
     else echo control=refused; fi; \
     if printf probe > \"$d/scenario-probe\" 2>/dev/null; then echo probe=wrote; \
     else echo probe=refused; fi";

/// The output chat `id`'s command printed for `step`, once it played.
#[cfg(unix)]
fn stdout_of(scenario: &Scenario, id: u32, step: &str) -> String {
    scenario
        .result_of(id, step)
        .and_then(|result| result["stdout"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// A record of the fixture's for this leg's report.
#[cfg(unix)]
fn a_fixture_record(
    test: &str,
    dialect: Option<purlis_scenarios::model::Dialect>,
    protection: Option<purlis_scenarios::model::Protection>,
    verdict: purlis_scenarios::model::ReportVerdict,
    reason: &str,
) -> purlis_scenarios::model::LegRecord {
    a_record_of("T1.1-06", test, dialect, protection, verdict, reason)
}

/// [`a_fixture_record`] for the ticket `row` that wrote the test.
#[cfg(unix)]
fn a_record_of(
    row: &str,
    test: &str,
    dialect: Option<purlis_scenarios::model::Dialect>,
    protection: Option<purlis_scenarios::model::Protection>,
    verdict: purlis_scenarios::model::ReportVerdict,
    reason: &str,
) -> purlis_scenarios::model::LegRecord {
    purlis_scenarios::model::LegRecord {
        kind: purlis_scenarios::model::EntryKind::Fixture,
        row: row.to_owned(),
        test: Some(test.to_owned()),
        dialect,
        protection,
        os: report::this_os(),
        leg: purlis_scenarios::model::ReportLeg::Fixture,
        timings: Some(purlis_scenarios::model::TimingsMode::Scaled),
        verdict,
        reason: Some(reason.to_owned()),
        refusal_expected: false,
    }
}

/// **A scenario leaves no process and no temp folder behind** (T1.1-06): once its child ends,
/// nothing it started runs, a process its chat left in the background included, and every
/// folder of its own is gone (the parent's check, `Homes::of_this_scenario`).
#[cfg(unix)]
#[test]
fn a_scenario_leaves_no_process_and_no_temp_dir_behind() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(scenario) = Scenario::new(&homes, Protection::Full, None, Dialect::Codex) else {
        return;
    };
    let chat = scenario.starts(&serde_json::json!({
        "turns": [{"prompt": "go", "calls": [
            {"step": "where", "tool": "bash", "run": WHERE_AM_I},
            {"step": "sleeps", "tool": "bash",
             "run": format!(
                 "sleep 600 >/dev/null 2>&1 & echo $! > {}; echo started",
                 scenario.root.join(".scenario/sleeper.pid").display()
             )}
        ]}]
    }));
    scenario.until_played(chat, &["sleeps"]);
    let sleeper: u32 = std::fs::read_to_string(scenario.root.join(".scenario/sleeper.pid"))
        .unwrap_or_else(|why| {
            panic!(
                "the chat said nothing of what it left running ({why})\n{}",
                scenario.said()
            )
        })
        .trim()
        .parse()
        .expect("a process id");
    assert!(
        purlis_core::process::alive(sleeper),
        "it runs while the scenario does"
    );
    scenario.left_in_the_background(sleeper);
    assert!(!scenario.pids().is_empty(), "the chat runs");
    drop(scenario);
    // #7103 is the sleeper outliving the quit on macOS. When it stops doing so the expected
    // fail is retired, never left to hide a leak: the leg's report says so, and the test fails.
    if cfg!(target_os = "macos") && homes.background_left().is_empty() {
        let said = "unexpected pass #7103: the host's quit ended the chat's background process on \
                    macOS; retire the expected fail";
        report::write(&[a_fixture_record(
            "a_scenario_leaves_no_process_and_no_temp_dir_behind",
            Some(purlis_scenarios::model::Dialect::Codex),
            Some(purlis_scenarios::model::Protection::Full),
            purlis_scenarios::model::ReportVerdict::Fail,
            said,
        )])
        .expect("the leg's report");
        panic!("{said}");
    }
}

/// **A chat that outlives its host's quit fails the leak check on every OS** (T1.1-06, #7103):
/// on macOS a group the quit left is #7103's expected fail only when a test registered it as a
/// chat's background process's and every chat process seen in it has ended. Probe 2 (run
/// 44169502700) passed six macOS scenarios with a quit that ended nothing, because any group
/// left was taken for #7103's.
#[cfg(unix)]
#[test]
fn a_chat_that_outlives_its_hosts_quit_fails_the_leak_check_on_every_os() {
    let seen = [
        (100, Some(10)),
        (200, Some(20)),
        (201, Some(20)),
        (300, None),
    ];
    let dead = |_: u32| false;
    let chat_200_runs = |pid: u32| pid == 200;
    assert_eq!(
        host::of_the_quit(&[10, 20], &seen, &[10, 20], dead, true),
        (vec![10, 20], vec![]),
        "macOS: chats ended, only a registered background process left in each group: #7103's"
    );
    assert_eq!(
        host::of_the_quit(&[10, 20], &seen, &[10, 20], chat_200_runs, true),
        (vec![10], vec![20]),
        "macOS: a chat the quit did not end is a leak, not #7103's"
    );
    assert_eq!(
        host::of_the_quit(&[10, 20], &seen, &[10], dead, true),
        (vec![10], vec![20]),
        "macOS: a group no test registered as a background process's is a leak"
    );
    assert_eq!(
        host::of_the_quit(&[10, 20], &seen, &[10, 20], dead, false),
        (vec![], vec![10, 20]),
        "Linux: every group the quit left is a leak"
    );
}

/// **A group no chat was ever seen in is #7103's only when a test registered it** (T1.1-06,
/// #7103): a tool's command runs in a process group of its own, so "every chat seen in it has
/// ended" holds of it vacuously. Before #7152 any such group the quit left was taken for #7103's
/// on macOS, a held `curl` or a sleeper nobody named included.
#[cfg(unix)]
#[test]
fn a_group_no_chat_was_seen_in_is_a_leak_unless_registered_as_background() {
    let seen = [(100, Some(10))];
    let dead = |_: u32| false;
    assert_eq!(
        host::of_the_quit(&[30], &seen, &[], dead, true),
        (vec![], vec![30]),
        "macOS: a tool's group nobody registered, with no chat in it, is a leak"
    );
    assert_eq!(
        host::of_the_quit(&[30], &seen, &[30], dead, true),
        (vec![30], vec![]),
        "macOS: the same group, registered as a chat's background process's, is #7103's"
    );
    assert_eq!(
        host::of_the_quit(&[30], &[], &[], dead, true),
        (vec![], vec![30]),
        "macOS: with no chat seen at all, nothing is excused"
    );
}

/// **A held connection in a scenario raises an ask and blocks at the hold** (T1.1-06): the first
/// end-to-end smoke, in every dialect at Full. A chat asks for a host nothing lists; its proxy in
/// the session host holds the connection and the window is asked (the chat is held on the host
/// in the window's registry of asks); nobody answers,
/// so at the scaled hold the call is refused, never carried: the host is never looked up,
/// dialled or reached. It shows today's behaviour: the block itself is written at the next
/// prompt (#6673), and `ask.raised` may be written twice (#6674), so neither is pinned here.
#[cfg(unix)]
#[test]
fn a_held_connection_in_a_scenario_raises_an_ask_and_blocks_at_the_hold() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    for dialect in Dialect::ALL {
        // On macOS a Claude Code chat runs in Claude Code's own sandbox, as production runs it,
        // which the fake harness does not play: that is ticket 10's leg, the real binary under
        // it. purlis's wrap holds a Claude Code chat on Linux alone (`claude_wrap::wrap`), so
        // the dialect is reported not run here, never passed.
        if dialect == Dialect::Claude && cfg!(target_os = "macos") {
            report::write(&[a_fixture_record(
                "a_held_connection_in_a_scenario_raises_an_ask_and_blocks_at_the_hold",
                Some(purlis_scenarios::model::Dialect::Claude),
                Some(purlis_scenarios::model::Protection::Full),
                purlis_scenarios::model::ReportVerdict::NotRun,
                "Claude Code on macOS runs its own sandbox, which the fake does not play \
                 (ticket 10, #6900)",
            )])
            .expect("the leg's report");
            continue;
        }
        let Some(scenario) = Scenario::new(&homes, Protection::Full, Some("steward"), dialect)
        else {
            return;
        };
        let held = format!("held-{}.example.test", dialect.word());
        let asked_before = scenario.count_of(purlis_core::eventlog::stops::ASK_RAISED);
        let chat = scenario.starts(&serde_json::json!({
            "turns": [{"prompt": "go", "calls": [{
                "step": "held", "tool": "bash",
                "run": reaches(&held)
            }]}]
        }));
        // The window is asked while the call is held: the chat is held on the host in the
        // window's own registry of asks, what the person's Notice draws from.
        until(
            &scenario,
            "the window is asked while the call is held",
            || held_on(&scenario, chat, &held).is_some(),
        );
        scenario.until_played(chat, &["held"]);
        // The call's own time, as curl measured it: nothing of the chat's start-up.
        let took = took_of(&scenario, chat, "held");
        // Today's behaviour (B0, #6672, #6673): `ask.raised` can reach the event log well after
        // the call it held was refused (on Codex, as the chat's run ends), so it is not waited
        // for here; how late it was, where it came, is said.
        let _ = std::io::Write::write_fmt(
            &mut std::io::stderr(),
            format_args!(
                "{dialect:?}: the call ended after {took:?}; ask.raised in the log by then: {}\n",
                scenario.count_of(purlis_core::eventlog::stops::ASK_RAISED) > asked_before
            ),
        );
        assert!(
            took >= scenario.timings.hold(),
            "{dialect:?}: the call ended after {took:?}, before the hold of {:?} ended",
            scenario.timings.hold()
        );
        assert!(
            took < scenario.timings.hold() + HOLD_ENDS_WITHIN,
            "{dialect:?}: the call ended after {took:?}, not at the hold of {:?} (within {:?})",
            scenario.timings.hold(),
            HOLD_ENDS_WITHIN
        );
        let result = scenario.result_of(chat, "held").expect("played");
        assert!(
            !result.to_string().contains("status 200"),
            "{dialect:?}: the held connection was carried: {result}"
        );
        assert!(
            scenario.names.lookups().iter().all(|one| one.name != held),
            "{dialect:?}: a host nobody allowed was looked up: {:?}",
            scenario.names.lookups()
        );
        assert!(scenario.names.origin_hits().is_empty());
        // Nobody answered, so the ask is still listed, as a timed-out ask is: the person now
        // keeps it blocked, which the global checks need of a scenario that raised one.
        scenario.keeps_listed_asks_blocked();
    }
}

/// How soon after its hold a held call is refused: the board's refusal reaches curl at once, so
/// this is a slow runner's margin, well below curl's own `--max-time`.
#[cfg(unix)]
const HOLD_ENDS_WITHIN: Duration = Duration::from_secs(30);

/// The command a chat runs to reach `host` on HTTPS, saying the status it was answered and how
/// long the call took, as curl measures it.
#[cfg(unix)]
fn reaches(host: &str) -> String {
    format!(
        "curl -sS -o /dev/null -w 'status %{{http_code}} took %{{time_total}}' --max-time 300 \
         https://{host}/"
    )
}

/// How long chat `id`'s call of `step` ([`reaches`]) took, as curl measured it: from the
/// command's start, so never a chat's start-up.
#[cfg(unix)]
fn took_of(scenario: &Scenario, id: u32, step: &str) -> Duration {
    let said = stdout_of(scenario, id, step);
    said.split_once("took ")
        .and_then(|(_, seconds)| seconds.trim().parse::<f64>().ok())
        .map(Duration::from_secs_f64)
        .unwrap_or_else(|| panic!("chat {id}'s {step} said no time: {said:?}"))
}

/// The block chat `session` is held on now for `target`, as its Notice shows it.
#[cfg(unix)]
fn held_on(
    scenario: &Scenario,
    session: u32,
    target: &str,
) -> Option<crate::taskblocks::BlockShown> {
    // As the window reads them (T1.1-07): the Notices' standing blocks, which also record the
    // ask for the one-ask, one-raise check.
    scenario
        .queries()
        .notices()
        .standing
        .into_iter()
        .find(|(held, block)| *held == session && block.target.contains(target))
        .map(|(_, block)| block)
}

/// **Two chats in one host keep their grants apart** (T1.1-06, §3.7): the person allows a held
/// host for one chat alone, through the window's own command (`allow_sandbox_block`); that chat's
/// held connection is carried (its proxy looks the host up), and the other chat, asking for the
/// same host in the same host process, is held and refused at the hold, never carried.
#[cfg(unix)]
#[test]
fn two_chats_in_one_host_keep_their_grants_apart() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(scenario) = Scenario::new(&homes, Protection::Full, Some("steward"), Dialect::Codex)
    else {
        return;
    };
    // #6674: a held network ask is raised twice, by two writers, with nothing between them; the
    // one-ask, one-raise check finds it in the chat that nobody answers.
    scenario.expected_fail(
        checks::Check::OneAskOneRaise,
        6674,
        "a held network ask writes ask.raised twice",
    );
    let host = "granted-to-one.example.test";
    let script = |step: &str| {
        serde_json::json!({
            "turns": [{"prompt": "go", "calls": [{"step": step, "tool": "bash", "run": reaches(host)}]}]
        })
    };
    let lookups = || {
        scenario
            .names
            .lookups()
            .iter()
            .filter(|one| one.name == host)
            .count()
    };

    let one = scenario.starts(&script("reaches-one"));
    until(&scenario, "the first chat is held on the host", || {
        held_on(&scenario, one, host).is_some()
    });
    let shown = held_on(&scenario, one, host).expect("held");
    // Every ask is put through the same checks `when_listed` makes: safe to read, and the target
    // that is decided shown whole.
    if let Err(why) = checks::shows_the_decided_target(&shown, host) {
        scenario.fails_at("the first chat's ask", host, &why);
    }
    scenario
        .person()
        .allow_block(one, shown, crate::sandboxing::GrantLevel::Chat)
        .expect("the person allows it for this chat");
    scenario.until_played(one, &["reaches-one"]);
    until(&scenario, "the first chat's proxy carries it", || {
        lookups() > 0
    });
    let carried = lookups();

    let other = scenario.starts(&script("reaches-other"));
    until(&scenario, "the other chat is held on the host", || {
        held_on(&scenario, other, host).is_some()
    });
    let held_other = held_on(&scenario, other, host).expect("held");
    if let Err(why) = checks::shows_the_decided_target(&held_other, host) {
        scenario.fails_at("the other chat's ask", host, &why);
    }
    scenario.until_played(other, &["reaches-other"]);
    assert!(
        took_of(&scenario, other, "reaches-other") >= scenario.timings.hold(),
        "the other chat's call ended before the hold did: {:?}\n{}",
        scenario.result_of(other, "reaches-other"),
        scenario.said()
    );
    assert_eq!(
        lookups(),
        carried,
        "the other chat's proxy looked the host up: the first chat's grant reached it"
    );
    // Nothing sweeps the first chat's asks: if its allowed block is still listed, the check
    // NoUnansweredAsk reads it at the end (#7232).
    // The other chat's ask is still listed, as a timed-out ask is: the person keeps it blocked,
    // that chat's own ask and no other.
    let (_, kept) = scenario.person().when_listed(
        other,
        host,
        &person::Press::Keep(None),
        Duration::from_millis(50),
    );
    kept.unwrap_or_else(|why| panic!("{why}\n{}", scenario.said()));
}

/// **Where a chat's command runs**, as it says itself: its folder, whether it is held by
/// seccomp and no-new-privileges, its user namespace beside the first process's, and the command
/// lines of its ancestors. What a red run's reader needs to tell a wrapped chat from one that is
/// not (Linux; elsewhere what the system says).
#[cfg(unix)]
const WHERE_AM_I: &str = "echo cwd=$(pwd); grep -E 'Seccomp|NoNewPrivs' /proc/self/status 2>/dev/null; \
     echo ns=$(readlink /proc/self/ns/user 2>/dev/null) init=$(readlink /proc/1/ns/user 2>/dev/null); \
     p=$$; for i in 1 2 3 4 5; do p=$(awk '/^PPid/{print $2}' /proc/$p/status 2>/dev/null); \
     [ -n \"$p\" ] || break; echo \"up$i: $(tr '\\0' ' ' < /proc/$p/cmdline 2>/dev/null | cut -c1-300)\"; done";

/// A chat's two turns: the first played before the restart, the second typed after it.
#[cfg(unix)]
fn two_turns(first: &str, second: &str) -> serde_json::Value {
    serde_json::json!({
        "turns": [
            {"prompt": "first", "calls": [{"step": first, "tool": "bash", "run": "echo one"}]},
            {"prompt": "second", "calls": [{"step": second, "tool": "bash", "run": "echo two"}]}
        ]
    })
}

/// **A crashed window reattaches to the surviving host and chat** (T1.1-06, `restart: window`):
/// the window's links are cut as its crash cuts them and the window goes; the chat lives on in
/// the host, its process the same; a new window on the same homes takes it back, and what the
/// person types next reaches the same chat. What else survives is asserted by the cells
/// (ticket 43's expectations).
#[cfg(unix)]
#[test]
fn a_crashed_window_reattaches_to_the_surviving_host_and_chat() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(mut scenario) = Scenario::new(&homes, Protection::Full, None, Dialect::Codex) else {
        return;
    };
    let chat = scenario.starts(&two_turns("before-the-crash", "after-the-crash"));
    scenario.until_played(chat, &["before-the-crash"]);
    let before = scenario.open_chats();
    let pid = before
        .iter()
        .find(|(id, _)| *id == chat)
        .and_then(|(_, pid)| *pid)
        .expect("the chat runs");

    scenario.the_window_crashes_and_comes_back();
    assert!(
        purlis_core::process::alive(pid),
        "the chat outlived the window"
    );
    until(&scenario, "the new window has the chat back", || {
        scenario
            .open_chats()
            .iter()
            .any(|(_, back)| *back == Some(pid))
    });
    let (back, _) = scenario
        .open_chats()
        .into_iter()
        .find(|(_, back)| *back == Some(pid))
        .expect("the same chat");
    scenario.types(back, "second");
    scenario.until_played(back, &["after-the-crash"]);
    assert_eq!(
        scenario.open_chats().len(),
        before.len(),
        "the chat was taken back, not started again beside itself"
    );
}

/// **A quit ends every chat and a relaunch restarts the live ones** (T1.1-06, `restart: quit`,
/// §3.17, M-BH-11): two chats run and the person closes the second; the window tells the host to
/// quit, the host answers once every chat has ended, and every chat process has ended before the
/// old host is dropped (`Scenario::quits_and_relaunches`); a new host and window start on the
/// same homes and relaunch the chat that was live, chat 1, once, in a process of its own, and
/// never the one the person closed.
#[cfg(unix)]
#[test]
fn a_quit_ends_every_chat_and_a_relaunch_restarts_the_live_ones() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(mut scenario) = Scenario::new(&homes, Protection::Full, None, Dialect::Codex) else {
        return;
    };
    let chat = scenario.starts(&two_turns("before-the-quit", "never-played-here"));
    scenario.until_played(chat, &["before-the-quit"]);
    let closed = scenario.starts(&two_turns("closed-before-the-quit", "never-played-there"));
    scenario.until_played(closed, &["closed-before-the-quit"]);
    let closed_pid = scenario
        .open_chats()
        .into_iter()
        .find(|(id, _)| *id == closed)
        .and_then(|(_, pid)| pid)
        .expect("the chat to close runs");
    scenario.closes(closed);
    until(&scenario, "the closed chat is no longer open", || {
        scenario.open_chats().iter().all(|(id, _)| *id != closed)
    });
    until(
        &scenario,
        "the closed chat's process ended before the quit",
        || !purlis_core::process::alive(closed_pid),
    );
    assert_eq!(
        scenario.open_names(),
        ["chat 1"],
        "only chat 1 is live before the quit"
    );
    let before = scenario.pids();
    assert!(!before.is_empty(), "the chat runs");

    scenario.quits_and_relaunches();
    for pid in &before {
        assert!(
            !purlis_core::process::alive(*pid),
            "the quit ended every chat: {pid} still runs"
        );
    }
    until(&scenario, "the relaunch restarts the live chat", || {
        scenario
            .pids()
            .iter()
            .any(|pid| !before.contains(pid) && purlis_core::process::alive(*pid))
    });
    assert_eq!(
        scenario.open_names(),
        ["chat 1"],
        "the one live chat came back, once, and the closed one did not\n{}",
        scenario.said()
    );
    // What the relaunched chat plays is not this scenario's: a relaunch composes its start from
    // the reopen record, which keeps no variable of the chat's, so its turn script is the
    // pinned one (T1.1-04's shim). That it plays on from its next turn is ticket 7's cell.
}

/// **The host kill is observed** (T1.1-06, `restart: host_killed`): on Linux, in a session host
/// of its own process, the `purlis-app` CI built apart from the test build (`PURLIS_HOST_APP`),
/// at production timings, the host is killed outright, and the window is told its chat ended
/// with its host. What survives and what is reported beyond that are ticket 48's cells.
/// Elsewhere the host is never its own process in the scenarios (`PURLIS_HOST_APP` is built on
/// Linux alone), so the leg reports it `not_run`, never passed.
#[cfg(unix)]
#[test]
fn the_host_kill_is_observed() {
    if !cfg!(target_os = "linux") {
        report::write(&[a_fixture_record(
            "the_host_kill_is_observed",
            None,
            None,
            purlis_scenarios::model::ReportVerdict::NotRun,
            "the session host runs as a process of its own in the scenarios on Linux alone \
             (`PURLIS_HOST_APP`)",
        )])
        .expect("the leg's report");
        return;
    }
    let Some(app) = std::env::var_os(host::HOST_APP_VAR) else {
        return host::cannot_run::<()>(&format!(
            "{} names no purlis-app built apart from the test build",
            host::HOST_APP_VAR
        ))
        .unwrap_or(());
    };
    assert_eq!(built_with(Path::new(&app)), AS_RELEASED, "the host process");
    let home = tempfile::tempdir().expect("the host's home");
    let target = crate::behaviour::in_a_host_process_running(home.path(), app.into());
    let ends = crate::behaviour::told_of_ends(&target);
    let chat = target
        .sessions
        .open(
            None,
            &crate::behaviour::a_shell("printf alive.; read line"),
            &|_| {},
        )
        .expect("the chat starts in the host process");
    crate::behaviour::until("the chat runs", || {
        target.sessions.process_id(chat).is_some()
    });

    (target.kill_the_host.as_ref().expect("a host process"))();

    crate::behaviour::until("the window is told the chat ended with its host", || {
        ends.lock()
            .expect("the ends")
            .iter()
            .any(|(id, _)| *id == chat)
    });
    assert!(
        target.sessions.lost_with_its_host(chat),
        "the chat's end is the host's, as the window reports it"
    );
}

/// **The maximal chat holds every grant kind** (T1.1-06): one chat holds a grant of every
/// `grant::What` variant (host, write, socket, service, port, listen), as the person allowed
/// them for it, and each is asserted on the chat itself: its sandbox as the window composed it
/// names every one, its confinement in the host lists the granted host, and its own proxy
/// carries that host at once, never held. It runs confined with an ssh route beside its proxy,
/// and a tunnel on the chat's own reach to the granted host is dialled through the scenario's
/// seam. A variant added to `grant::What` fails to compile in `host::kind_of`, and once it is
/// named there, fails here until the maximal chat holds one. The chat's open browser is
/// reported `not_run` (#7100); `purlis secret exec` run by the chat itself waits on the vault
/// broker fixture (#7101).
#[cfg(unix)]
#[test]
fn the_maximal_chat_holds_every_grant_kind() {
    use purlis_core::sandbox::grant::What;
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(scenario) = Scenario::new(&homes, Protection::Full, Some("steward"), Dialect::Codex)
    else {
        return;
    };
    // A socket a grant can name: in a folder of the person's own that no chat may write, never
    // the project or the temp folder (`services::SocketGround`).
    let sockets = scenario.a_folder_outside("sockets");
    let every = host::one_of_every_kind(&scenario.root, &sockets);
    let kinds: std::collections::BTreeSet<usize> = every.iter().map(host::kind_of).collect();
    assert_eq!(
        kinds,
        (0..host::GRANT_KINDS).collect(),
        "the maximal chat holds no grant of a `grant::What` variant"
    );
    let mut grants = purlis_core::sandbox::grant::Grants::default();
    for what in &every {
        grants.add(what);
    }
    let granted_host = "granted.example.test";
    let chat = scenario.starts_holding(
        &serde_json::json!({
            "turns": [{"prompt": "go", "calls": [
                {"step": "maximal", "tool": "bash", "run": "echo held"},
                {"step": "reaches-granted", "tool": "bash", "run": reaches(granted_host)}
            ]}]
        }),
        grants,
    );
    let began = std::time::Instant::now();
    scenario.until_played(chat, &["reaches-granted"]);
    let said = scenario.said();

    // On the chat itself: its sandbox, as the window composed it at its start.
    let applied = scenario
        .applied_of(chat)
        .unwrap_or_else(|| panic!("the maximal chat runs with no sandbox\n{said}"));
    let composed = format!("{applied:?}");
    for what in &every {
        let named = match what {
            What::Host(host) => host.to_string(),
            What::Write(path) | What::Socket(path) => path.display().to_string(),
            What::Service(name) => name.clone(),
            What::Port(port) | What::Listen(port) => port.to_string(),
        };
        assert!(
            composed.contains(&named),
            "the maximal chat's sandbox does not hold its {what:?} grant ({named})\n{said}"
        );
    }
    // Its confinement in the host, and its own proxy, which carries the granted host at once.
    let started = scenario.confines.started();
    let ours = started
        .last()
        .unwrap_or_else(|| panic!("the maximal chat runs unconfined\n{said}"));
    assert!(
        ours.plan.listed.iter().any(|one| one.host == granted_host),
        "the chat's confinement does not list its granted host: {:?}",
        ours.plan
    );
    assert!(
        ours.ssh.is_some(),
        "an ssh route beside its proxy: {started:?}"
    );
    assert!(
        scenario
            .names
            .lookups()
            .iter()
            .any(|one| one.name == granted_host),
        "the chat's proxy did not carry its granted host: {:?}\n{said}",
        scenario.names.lookups()
    );
    assert!(
        began.elapsed() < scenario.timings.hold(),
        "the chat's call to its granted host was held: {:?}",
        began.elapsed()
    );
    assert!(
        held_on(&scenario, chat, granted_host).is_none(),
        "the chat was asked about a host it holds"
    );

    // A tunnel on the chat's own reach, to the host it was granted, through the seam.
    let origin = purlis_scenarios::net::Origin::http().expect("an origin");
    scenario.names.origin(granted_host, 5432, origin.addr());
    let _held = scenario.names.carry_tunnels();
    let tunnel = purlis_core::sandbox::tunnel::Tunnel::open(
        &purlis_core::sandbox::tunnel::pointed(&format!("{granted_host}:5432")).expect("a target"),
        &purlis_core::sandbox::reach::Decision::Allowed(purlis_core::sandbox::reach::By::Chat),
        applied.confines().reach.clone(),
        None,
    )
    .expect("the tunnel opens on the chat's reach");
    let _ = std::net::TcpStream::connect(("127.0.0.1", tunnel.port())).map(|mut stream| {
        use std::io::{Read as _, Write as _};
        let _ = write!(
            stream,
            "GET / HTTP/1.1\r\nHost: {granted_host}\r\nConnection: close\r\n\r\n"
        );
        let _ = stream.read_to_end(&mut Vec::new());
    });
    assert_eq!(
        scenario.names.origin_hits(),
        vec![(granted_host.to_owned(), 5432)]
    );

    report::write(&[a_fixture_record(
        "the_maximal_chat_holds_every_grant_kind",
        Some(purlis_scenarios::model::Dialect::Codex),
        Some(purlis_scenarios::model::Protection::Full),
        purlis_scenarios::model::ReportVerdict::NotRun,
        "the maximal chat's open browser: the scenario host has no browser driver yet (#7100)",
    )])
    .expect("the leg's report");
}

/// Until `done`, within [`PLAYS_WITHIN`]: a chat's harness starts and plays first.
#[cfg(unix)]
fn until(scenario: &Scenario, what: &str, done: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + PLAYS_WITHIN;
    while !done() {
        assert!(
            std::time::Instant::now() < deadline,
            "{what} never happened\n{}",
            scenario.said()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// **The scenario host never uses `testgit`'s config** (T1.1-06): it sets
/// `safe.bareRepository=all` (`purlis_core::testgit`), which would mask ticket 21's
/// embedded-bare-repo cell. Neither the scenario host's sources nor the suite's name it.
#[test]
fn the_scenario_host_never_uses_testgits_config() {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for file in [
        "scenarios.rs",
        "scenarios/host.rs",
        "scenarios/report.rs",
        "scenarios/switches.rs",
    ] {
        let text = std::fs::read_to_string(here.join(file)).expect("a source");
        let named = text
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .any(|line| line.contains(concat!("test", "git::")));
        assert!(!named, "{file} uses testgit's config");
    }
}

/// How long a chat's script has to play.
#[cfg(unix)]
pub(crate) const PLAYS_WITHIN: Duration = Duration::from_secs(120);

/// The entries run at `Timings::production()` (ticket 54), picked by the `production-timings`
/// label and run in the merge queue.
mod production_timings {
    #[test]
    fn the_production_timing_entries_run_when_asked() {}

    /// **A production-timing run reports what it skips** (T1.1-54): the run's trigger
    /// (`SCENARIOS_TRIGGER`: `label`, `merge_group` or `nightly`; the label's when unset) takes the
    /// derived production-timing set, less the `production_nightly_only` cells (M-NET-25, Q300)
    /// off the nightly, which it writes to the leg report as skipped-for-timing, never as passed.
    /// The nightly skips none.
    #[cfg(unix)]
    #[test]
    fn the_production_timing_run_reports_what_it_skips() {
        use purlis_scenarios::legs::trigger_from;
        use purlis_scenarios::model::TimingsMode;

        let trigger = trigger_from(
            &std::env::var("SCENARIOS_TRIGGER").unwrap_or_else(|_| "label".to_owned()),
        )
        .expect("a trigger");
        for batch in super::report::every_batch() {
            let skipped =
                super::report::skipped_for_timing(&batch, TimingsMode::Production, trigger);
            super::report::write(&skipped).expect("the report is written");
        }
    }
}

#[cfg(unix)]
#[path = "scenarios/window_person.rs"]
mod window_person;
