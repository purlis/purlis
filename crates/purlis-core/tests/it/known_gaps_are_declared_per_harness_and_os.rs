// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Known gaps, declared per harness and OS** (M75 slice 37, #7046; spec §6, §14.8; Q319 C1).
//!
//! A rule in force that a harness cannot enforce is a known gap: declared, never refused, unless
//! the project writes the refusal as a forbid on `start`. These tests judge the capability table
//! against the schema, the entries against #6972 §4, and a start's refusal against a real plane
//! made with git in a temporary folder.

use std::collections::BTreeSet;
use std::path::Path;

use purlis_core::enforcement::{Family, Mode, Resolved};
use purlis_core::harness::Harness;
use purlis_core::knowngaps::{self, Cover, Declared, Facts, GAPS, Judged, Ruling};
use purlis_core::policy::levels::Chat;
use purlis_core::sandbox::Os;

const SYSTEMS: [Os; 4] = [Os::MacOs, Os::Linux, Os::Windows, Os::Other];

fn git(dir: &Path, args: &[&str]) {
    let ran = purlis_core::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .expect("git runs");
    assert!(ran.status.success(), "git {args:?}: {ran:?}");
}

/// A plane whose root commits `policy.cedar` with `rules`, or none.
fn plane(rules: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    git(dir.path(), &["init", "-q"]);
    std::fs::write(dir.path().join("README.md"), "a plane\n").expect("a file");
    if let Some(rules) = rules {
        std::fs::write(
            dir.path().join("policy.cedar"),
            format!("// purlis-policy 1\n{rules}"),
        )
        .expect("a file");
    }
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "c"]);
    dir
}

fn facts(harness: Harness, os: Os) -> Facts {
    Facts::best(harness, os)
}

/// What a start's refusal says, where the rule refuses it.
fn refused(root: &Path, facts: &Facts, declared: &Declared, floor: &Resolved) -> Option<String> {
    match knowngaps::refusal(&judged(root), facts, declared, floor) {
        Some(Ruling::Refused(said)) => Some(said),
        _ => None,
    }
}

fn judged(root: &Path) -> Judged<'_> {
    Judged {
        root,
        profile: "claude",
        of: Chat {
            workspace: None,
            persona: None,
        },
        sandbox: "floor",
        attended: true,
        harness_mode: Some(purlis_core::attendance::ClaudeCodeMode::Default),
    }
}

/// `floor` with `family` at `mode`, every other family enforced.
fn floor_with(family: Family, mode: Mode) -> Resolved {
    let text = Family::ALL
        .into_iter()
        .map(|each| {
            let mode = if each == family { mode } else { Mode::Enforce };
            format!("{}={}", each.key(), mode.word())
        })
        .collect::<Vec<_>>()
        .join(",");
    let parsed = Resolved::parse(&text);
    assert_eq!(parsed.mode(family), mode, "the floor spells {text}");
    parsed
}

const REFUSE_ON_A_SHAPE_GAP: &str = r#"
@id("trust-no-start-on-a-shape-gap") @level("project")
forbid (principal, action == Action::"start", resource)
  when { context.gaps.contains("guards.shape") };
"#;

#[test]
fn a_start_whose_policy_needs_a_hook_the_harness_lacks_declares_the_gap_and_starts() {
    purlis_core::unsteered!();
    // Codex has no fail-closed point for the shape of a call but its hook, and the shipped
    // default enforces it.
    let plane = plane(None);
    let floor = Resolved::all(Mode::Enforce);
    let facts = facts(Harness::Codex, Os::Linux);
    let declared = Declared::at(&facts, &floor);
    assert!(declared.ids().contains(&knowngaps::HOOK_FAILS_OPEN));
    assert!(declared.context_gaps().contains(&"guards.shape".to_owned()));
    // The shipped default has no rule on `start`: the chat starts.
    assert_eq!(
        knowngaps::refusal(&judged(plane.path()), &facts, &declared, &floor),
        None,
        "no known gap refuses a start under the shipped default"
    );
}

#[test]
fn a_project_rule_on_start_with_a_gap_in_an_enforced_family_refuses_naming_the_rule() {
    purlis_core::unsteered!();
    let plane = plane(Some(REFUSE_ON_A_SHAPE_GAP));
    let floor = Resolved::all(Mode::Enforce);
    let facts = facts(Harness::Codex, Os::Linux);
    let declared = Declared::at(&facts, &floor);
    let refused = refused(plane.path(), &facts, &declared, &floor)
        .expect("the project's rule refuses a start with a gap in a family it enforces");
    assert!(
        refused.contains("trust-no-start-on-a-shape-gap"),
        "names the rule: {refused}"
    );
    assert!(
        refused.contains(knowngaps::HOOK_FAILS_OPEN),
        "names the gap: {refused}"
    );
    assert!(refused.starts_with("Nothing was started."), "{refused}");
}

#[test]
fn a_gap_in_a_family_set_to_log_or_off_is_not_in_context_gaps() {
    purlis_core::unsteered!();
    let facts = facts(Harness::Codex, Os::Linux);
    let enforced = Declared::at(&facts, &Resolved::all(Mode::Enforce));
    assert!(enforced.context_gaps().contains(&"guards.shape".to_owned()));

    for mode in [Mode::Log, Mode::Off] {
        let declared = Declared::at(&facts, &floor_with(Family::GuardsShape, mode));
        assert!(
            !declared.context_gaps().contains(&"guards.shape".to_owned()),
            "a family at {mode:?} is not enforced, so it is not in context.gaps"
        );
    }

    // And a project rule that refuses on it refuses no start.
    let plane = plane(Some(REFUSE_ON_A_SHAPE_GAP));
    for mode in [Mode::Log, Mode::Off] {
        let floor = floor_with(Family::GuardsShape, mode);
        let declared = Declared::at(&facts, &floor);
        assert_eq!(
            knowngaps::refusal(&judged(plane.path()), &facts, &declared, &floor),
            None
        );
    }
}

#[test]
fn an_off_rule_needs_no_enforcement_and_declares_no_gap() {
    purlis_core::unsteered!();
    let facts = facts(Harness::Codex, Os::Linux);
    let off = Declared::at(&facts, &floor_with(Family::Asks, Mode::Off));
    assert!(
        off.entries()
            .iter()
            .all(|entry| !entry.in_force.contains(&Family::Asks)),
        "no declared gap names the family that is off"
    );
    // The Codex gaps that affect only `asks` are gone, and the others stay.
    for id in ["codex-no-command-ask", "codex-sandboxed-asks-rejected"] {
        assert!(!off.ids().contains(&id), "{id} needs enforcement of `asks`");
    }
    let on = Declared::at(&facts, &Resolved::all(Mode::Enforce));
    for id in ["codex-no-command-ask", "codex-sandboxed-asks-rejected"] {
        assert!(
            on.ids().contains(&id),
            "{id} is declared where `asks` is on"
        );
    }
    // A `log` family still declares: it is in force, only not enforced.
    let logging = Declared::at(&facts, &floor_with(Family::Asks, Mode::Log));
    assert!(logging.ids().contains(&"codex-no-command-ask"));
}

#[test]
fn every_gap_of_the_harness_research_has_an_entry() {
    purlis_core::unsteered!();
    let numbered: Vec<u8> = GAPS.iter().filter_map(|gap| gap.research).collect();
    assert_eq!(
        numbered,
        (1..=24).collect::<Vec<u8>>(),
        "#6972 §4's 24 gaps, each once, in its order"
    );
    let ids: BTreeSet<&str> = GAPS.iter().map(|gap| gap.id).collect();
    assert_eq!(ids.len(), GAPS.len(), "each gap has an id of its own");
    for gap in GAPS {
        assert!(!gap.said.is_empty() && !gap.instead.is_empty() && !gap.from.is_empty());
        assert!(
            gap.id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()),
            "{} is kebab-case",
            gap.id
        );
    }
}

/// Every way a start can stand: each harness and a declared one with no adapter, each system, the
/// sandbox held and lifted.
fn every_start() -> Vec<Facts> {
    let mut all = Vec::new();
    let harnesses = Harness::ALL.map(Some).into_iter().chain([None]);
    for harness in harnesses {
        for os in SYSTEMS {
            for sandboxed in [true, false] {
                all.push(Facts {
                    harness,
                    os,
                    sandboxed,
                    claude_code_older: false,
                    plane_root_rename_held: false,
                });
            }
        }
    }
    all
}

#[test]
fn every_action_has_a_fail_closed_point_or_a_gap_that_a_start_declares() {
    purlis_core::unsteered!();
    let actions = purlis_core::policy::actions();
    assert_eq!(actions.len(), 63, "the schema's actions: {actions:?}");
    let floor = Resolved::all(Mode::Enforce);
    for facts in every_start() {
        let declared = Declared::at(&facts, &floor);
        for &(action, family) in &actions {
            let family = Family::of_key(family).expect("a family of the schema");
            let cover = knowngaps::cover(action, &facts).unwrap_or_else(|| {
                panic!("`{action}` has no row in the capability table ({facts:?})")
            });
            let Cover::Gap(id) = cover else { continue };
            let gap = GAPS
                .iter()
                .find(|gap| gap.id == id)
                .unwrap_or_else(|| panic!("`{action}`'s gap `{id}` has no entry"));
            assert!(
                gap.holds_for(&facts),
                "`{id}` stands for `{action}` but does not hold for {facts:?}"
            );
            // The start declares the gap the table answers with, for the family of the action.
            let entry = declared
                .entries()
                .iter()
                .find(|entry| entry.gap.id == id)
                .unwrap_or_else(|| panic!("`{id}` is not declared for {facts:?}"));
            assert!(
                entry.in_force.contains(&family),
                "`{id}` is declared for {facts:?} without `{}`, which holds `{action}`",
                family.key()
            );
        }
    }
    assert_eq!(
        knowngaps::cover("no-such-action", &facts(Harness::Codex, Os::Linux)),
        None
    );
}

#[test]
fn the_capability_table_holds_what_the_sandbox_profile_holds_only_where_it_is_applied() {
    purlis_core::unsteered!();
    // Every `Closed(Sandbox | Proxy | Harness)` is a fact about an applied sandbox on a system
    // with a backend: lifted, or with none, it is the gap instead.
    for facts in every_start() {
        for (action, _) in purlis_core::policy::actions() {
            let cover = knowngaps::cover(action, &facts).expect("a row");
            let by_the_sandbox = matches!(
                cover,
                Cover::Closed(
                    knowngaps::Point::Sandbox | knowngaps::Point::Proxy | knowngaps::Point::Harness
                )
            );
            let held = facts.sandboxed && facts.os.backend_word().is_some();
            if by_the_sandbox && !held && facts.harness.is_some() {
                panic!("`{action}` is closed by the sandbox where {facts:?} holds none");
            }
            if by_the_sandbox && facts.harness.is_none() {
                assert!(held, "`{action}` is closed by a sandbox for {facts:?}");
            }
        }
    }
}

#[test]
fn a_lifted_sandbox_is_a_gap_for_every_family_the_sandbox_holds() {
    purlis_core::unsteered!();
    let floor = Resolved::all(Mode::Enforce);
    for harness in Harness::ALL {
        let held = Facts::best(harness, Os::Linux);
        let lifted = Facts {
            sandboxed: false,
            ..held
        };
        assert!(
            !Declared::at(&held, &floor)
                .ids()
                .contains(&knowngaps::SANDBOX_LIFTED)
        );
        let declared = Declared::at(&lifted, &floor);
        let entry = declared
            .entries()
            .iter()
            .find(|entry| entry.gap.id == knowngaps::SANDBOX_LIFTED)
            .expect("a lifted sandbox is a declared gap");
        for family in [
            Family::Sandbox,
            Family::Network,
            Family::Ports,
            Family::Processes,
        ] {
            assert!(entry.in_force.contains(&family), "{harness:?}: {family:?}");
        }
        for action in [
            "reach", "listen", "exec", "install", "rewrite", "create", "remove",
        ] {
            assert_eq!(
                knowngaps::cover(action, &lifted),
                Some(Cover::Gap(knowngaps::SANDBOX_LIFTED)),
                "{harness:?}: {action}"
            );
        }
    }
    // OpenCode's shim holds git's and the credential's guards only inside the sandbox.
    let lifted = Declared::at(
        &Facts {
            sandboxed: false,
            ..Facts::best(Harness::Opencode, Os::Linux)
        },
        &floor,
    );
    let context = lifted.context_gaps();
    for family in ["guards.git", "guards.credential", "ports"] {
        assert!(
            context.contains(&family.to_owned()),
            "{family}: {context:?}"
        );
    }
    let held = Declared::at(&Facts::best(Harness::Opencode, Os::Linux), &floor).context_gaps();
    assert!(!held.contains(&"guards.git".to_owned()), "{held:?}");
}

#[test]
fn a_declared_harness_with_no_adapter_has_every_enforced_family_as_a_gap() {
    purlis_core::unsteered!();
    let floor = Resolved::all(Mode::Enforce);
    let unadapted = Facts {
        harness: None,
        ..Facts::best(Harness::ClaudeCode, Os::Linux)
    };
    let declared = Declared::at(&unadapted, &floor);
    assert!(declared.ids().contains(&knowngaps::UNADAPTED_HARNESS));
    let context = declared.context_gaps();
    for family in Family::ALL {
        assert!(
            context.contains(&family.key().to_owned()),
            "`{}` is no gap of a harness with no adapter",
            family.key()
        );
    }
    // No harness of an adapter has it, in Settings or at a start.
    for harness in Harness::ALL {
        assert!(
            !Declared::on(harness, Os::Linux, &floor)
                .ids()
                .contains(&knowngaps::UNADAPTED_HARNESS)
        );
    }
}

#[test]
fn an_ask_on_start_refuses_and_says_asking_at_a_start_is_not_wired() {
    purlis_core::unsteered!();
    let plane = plane(Some(
        r#"
@id("trust-ask-on-a-shape-gap") @level("project") @ask("you")
forbid (principal, action == Action::"start", resource)
  when { context.gaps.contains("guards.shape") };
"#,
    ));
    let floor = Resolved::all(Mode::Enforce);
    let facts = facts(Harness::Codex, Os::Linux);
    let declared = Declared::at(&facts, &floor);
    let said = refused(plane.path(), &facts, &declared, &floor)
        .expect("an ask at a start refuses it, until asking at a start is wired");
    assert!(said.starts_with("Nothing was started."), "{said}");
    assert!(said.contains("trust-ask-on-a-shape-gap"), "{said}");
    assert!(said.contains("asks"), "{said}");
    assert!(said.contains("not wired yet"), "{said}");
}

#[test]
fn a_forbid_on_start_under_a_logged_trust_family_starts_with_the_would_have_blocked_line() {
    purlis_core::unsteered!();
    let plane = plane(Some(REFUSE_ON_A_SHAPE_GAP));
    // `trust` is enforced under the default template since Q329, so the project says `log`.
    std::fs::write(
        plane.path().join("charter.toml"),
        "schema = 1\n[policy]\ntrust = \"log\"\n",
    )
    .expect("settings");
    git(plane.path(), &["add", "-A"]);
    git(plane.path(), &["commit", "-q", "-m", "trust is logged"]);
    let facts = facts(Harness::Codex, Os::Linux);
    let floor = floor_with(Family::Trust, Mode::Log);
    let declared = Declared::at(&facts, &floor);
    match knowngaps::refusal(&judged(plane.path()), &facts, &declared, &floor) {
        Some(Ruling::Logged(line)) => {
            assert!(line.starts_with("Would have refused this start"), "{line}");
            assert!(line.contains("trust-no-start-on-a-shape-gap"), "{line}");
            assert!(line.contains("enforce"), "{line}");
        }
        other => panic!("a logged trust family writes the line and starts: {other:?}"),
    }
}

#[test]
fn a_system_with_no_sandbox_backend_declares_it_whatever_its_name() {
    purlis_core::unsteered!();
    let floor = Resolved::all(Mode::Enforce);
    for os in [Os::Windows, Os::Other] {
        let declared = Declared::at(&facts(Harness::Codex, os), &floor);
        assert!(
            declared.ids().contains(&knowngaps::NO_SANDBOX_BACKEND),
            "{os:?} has no sandbox backend"
        );
    }
    for os in [Os::MacOs, Os::Linux] {
        let declared = Declared::at(&facts(Harness::Codex, os), &floor);
        assert!(!declared.ids().contains(&knowngaps::NO_SANDBOX_BACKEND));
    }
}

#[test]
fn claude_code_reads_are_a_declared_gap() {
    purlis_core::unsteered!();
    let floor = Resolved::all(Mode::Enforce);
    for os in [Os::MacOs, Os::Linux] {
        let declared = Declared::at(&facts(Harness::ClaudeCode, os), &floor);
        assert!(declared.ids().contains(&"claude-code-reads"));
        assert!(declared.context_gaps().contains(&"sandbox".to_owned()));
    }
    let codex = Declared::at(&facts(Harness::Codex, Os::MacOs), &floor);
    assert!(!codex.ids().contains(&"claude-code-reads"));
}

#[test]
fn an_unheld_plane_root_rename_is_a_declared_gap() {
    purlis_core::unsteered!();
    let floor = Resolved::all(Mode::Enforce);
    let mut unheld = facts(Harness::Codex, Os::Linux);
    assert!(
        Declared::at(&unheld, &floor)
            .ids()
            .contains(&"plane-root-rename")
    );
    unheld.plane_root_rename_held = true;
    assert!(
        !Declared::at(&unheld, &floor)
            .ids()
            .contains(&"plane-root-rename")
    );
}

#[test]
fn an_unset_context_machine_is_a_declared_gap_everywhere_and_a_rule_reading_it_denies() {
    purlis_core::unsteered!();
    let floor = Resolved::all(Mode::Enforce);
    for os in SYSTEMS {
        let there = Declared::at(&facts(Harness::ClaudeCode, os), &floor);
        let gap = there
            .entries()
            .iter()
            .find(|entry| entry.gap.id == "machine-unset")
            .unwrap_or_else(|| panic!("declared on {os:?}"));
        assert!(
            gap.gap.families.is_empty(),
            "no family's mode switches it off"
        );
    }

    // A rule that reads `context.machine` (guarded, as the schema's optional attribute needs),
    // with the device id unset as it is on Windows, denies: what it expects is never there.
    let plane = plane(Some(
        r#"
@id("processes-this-machine-only") @level("project")
forbid (principal, action == Action::"exec", resource)
  unless { context has machine && context.machine == "01JABCDEFGHJKMNPQRSTVWXYZ0" };
"#,
    ));
    let asker = purlis_core::policy::decide::Asker {
        chat: 1,
        of: Chat {
            workspace: None,
            persona: None,
        },
        sandbox: "floor",
        attended: true,
        harness_mode: "default",
        depth: 0,
        via: "hook",
        start: None,
        git: None,
    };
    let decision = purlis_core::policy::decide::in_plane(
        plane.path(),
        &asker,
        "exec",
        ("Command", "ls"),
        &purlis_core::policy::decide::Sealed::unsealed(),
    );
    assert_eq!(
        decision.outcome,
        purlis_core::policy::decide::Outcome::Deny,
        "a rule that reads an unset machine denies: {decision:?}"
    );
    assert!(
        decision.ids().contains(&"processes-this-machine-only"),
        "{decision:?}"
    );
}

#[test]
fn a_declared_gap_shows_in_settings_doctor_and_the_pane_head() {
    purlis_core::unsteered!();
    // Doctor declares each entry that holds for a harness on this system. Settings and the pane
    // head read the same list through the app's commands; their stories and tests are the
    // window's (`KnownGaps.test.tsx`).
    let rows = purlis_core::doctor::known_gaps_rows(&Resolved::all(Mode::Enforce), Os::Linux);
    assert_eq!(rows.len(), Harness::ALL.len(), "a row for each harness");
    for harness in Harness::ALL {
        let row = rows
            .iter()
            .find(|row| row.name == format!("known gaps: {}", harness.name()))
            .unwrap_or_else(|| panic!("doctor has no row for {harness:?}"));
        for gap in GAPS
            .iter()
            .filter(|gap| gap.may_hold_for(harness, Os::Linux))
        {
            assert!(
                row.detail.contains(gap.id) && row.detail.contains(gap.said),
                "doctor does not declare `{}` for {harness:?}: {row:?}",
                gap.id
            );
        }
        assert_ne!(
            row.status,
            purlis_core::doctor::Status::Fail,
            "a known gap never fails a start or a doctor run"
        );
    }
    // A gap of a family that is off is not declared.
    let off = purlis_core::doctor::known_gaps_rows(&floor_with(Family::Asks, Mode::Off), Os::Linux);
    let codex = off
        .iter()
        .find(|row| row.name == "known gaps: codex")
        .expect("a row for codex");
    assert!(!codex.detail.contains("codex-no-command-ask"), "{codex:?}");
}

// ---- a real start, through `start::ready_on` -----------------------------------------------------

/// A project that enforces `trust` and the families a rule on `start` reads, with a committed
/// `policy.cedar` of `rules`, and a profile `work` of `kind` running a program that is never run.
#[cfg(unix)]
struct Project {
    dir: tempfile::TempDir,
    _outside: tempfile::TempDir,
}

#[cfg(unix)]
impl Project {
    fn new(kind: &str, settings: &str, rules: &str) -> Self {
        let dir = plane(Some(rules));
        std::fs::write(dir.path().join("charter.toml"), settings).expect("settings");
        std::fs::write(dir.path().join(".gitignore"), "/charter.local.toml\n").expect("ignore");
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "settings"]);
        // No sandbox is applied here, so the program need not sit in a folder no chat writes.
        let outside = tempfile::tempdir().expect("a folder");
        let program = outside.path().join(kind);
        std::fs::write(&program, "#!/bin/sh\nexit 0\n").expect("a program");
        std::os::unix::fs::PermissionsExt::set_mode(
            &mut std::fs::metadata(&program).expect("meta").permissions(),
            0o755,
        );
        std::fs::set_permissions(
            &program,
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .expect("executable");
        let program = program.display().to_string();
        std::fs::write(
            dir.path().join(purlis_core::profiles::LOCAL_FILE),
            format!("[harness.work]\nkind = {kind:?}\ncommand = [{program:?}]\n"),
        )
        .expect("a profile");
        let set = purlis_core::profiles::current(dir.path());
        let profile = set.get("work").expect("declared");
        purlis_core::profiletrust::record_launched(
            dir.path(),
            "work",
            &purlis_core::profiletrust::fingerprint(profile),
        )
        .expect("approved");
        Self {
            dir,
            _outside: outside,
        }
    }

    /// The start, as a machine that has no sandbox program installed makes it: no sandbox here.
    fn ready(&self) -> Result<purlis_core::start::Ready, String> {
        let start = purlis_core::start::Start {
            profile: Some("work".to_owned()),
            name: "ide.1".to_owned(),
            cwd: Some(self.dir.path().to_path_buf()),
            ..purlis_core::start::Start::default()
        };
        purlis_core::start::ready_on(
            &start,
            self.dir.path(),
            &purlis_core::harness_declaration::read(self.dir.path()),
            &purlis_core::sandbox::Machine::this(),
            &|_| false,
        )
    }
}

#[cfg(unix)]
const ENFORCE_TRUST: &str = "schema = 1\n[policy]\ntemplate = \"open\"\ntrust = \"enforce\"\n\
                             guards.git = \"enforce\"\n";

#[cfg(unix)]
const REFUSE_ON_A_GIT_GAP: &str = r#"
@id("trust-no-start-on-a-git-gap") @level("project")
forbid (principal, action == Action::"start", resource)
  when { context.gaps.contains("guards.git") };
"#;

#[cfg(unix)]
#[test]
fn a_real_start_of_each_harness_is_refused_by_a_project_forbid_on_start() {
    purlis_core::unsteered!();
    purlis_core::unsteered!();
    // `guards.git` is a gap of Claude Code and Codex for their hook alone, and of OpenCode where
    // its sandbox is lifted, as it is on a machine with no sandbox program.
    for kind in ["claude", "codex", "opencode"] {
        let project = Project::new(kind, ENFORCE_TRUST, REFUSE_ON_A_GIT_GAP);
        let refused = project.ready().expect_err(&format!(
            "a {kind} start the project's rule refuses is not started"
        ));
        assert!(
            refused.starts_with("Nothing was started."),
            "{kind}: {refused}"
        );
        assert!(
            refused.contains("trust-no-start-on-a-git-gap"),
            "{kind}: {refused}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_real_start_with_no_rule_on_start_is_never_refused_for_a_gap() {
    purlis_core::unsteered!();
    purlis_core::unsteered!();
    let project = Project::new("claude", ENFORCE_TRUST, "");
    project
        .ready()
        .expect("the shipped default refuses no start for a gap");
}
